use crate::SifliToolTrait;
use crate::WriteFlashFile;
use crate::common::ram_command::{Command, RamCommand, Response};
use crate::{ProgressCallback, ProgressInfo};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{BufReader, Read, Write};

/// 通用的Flash写入操作实现
pub struct FlashWriter;

impl FlashWriter {
    /// 擦除所有Flash区域
    pub fn erase_all<T>(
        tool: &mut T,
        write_flash_files: &[WriteFlashFile],
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let spinner = ProgressBar::new_spinner();
        if !tool.base().quiet {
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix("WRITE");
            spinner.set_message("Erasing all flash regions...");
        }
        let mut erase_address: Vec<u32> = Vec::new();
        for f in write_flash_files.iter() {
            let address = f.address & 0xFF00_0000;
            // 如果ERASE_ADDRESS中的地址已经被擦除过，则跳过
            if erase_address.contains(&address) {
                continue;
            }
            tool.command(Command::EraseAll { address: f.address })?;
            erase_address.push(address);
        }
        if !tool.base().quiet {
            spinner.finish_with_message("All flash regions erased");
        }
        Ok(())
    }

    /// 验证数据
    pub fn verify<T>(tool: &mut T, address: u32, len: u32, crc: u32) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let spinner = ProgressBar::new_spinner();
        if !tool.base().quiet {
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix("VERIFY");
            spinner.set_message("Verifying data...");
        }
        let response = tool.command(Command::Verify { address, len, crc })?;
        if response != Response::Ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Verify failed",
            ));
        }
        if !tool.base().quiet {
            spinner.finish_with_message("Verify success!");
        }
        Ok(())
    }

    /// 写入单个文件到Flash（非全擦除模式）
    pub fn write_file_incremental<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        step: &mut i32,
        verify: bool,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let re_download_spinner = ProgressBar::new_spinner();
        let download_bar = ProgressBar::new(file.file.metadata()?.len());

        let download_bar_template = ProgressStyle::default_bar()
            .template("[{prefix}] {msg} {wide_bar} {bytes_per_sec} {percent_precise}%")
            .unwrap()
            .progress_chars("=>-");

        if !tool.base().quiet {
            re_download_spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            re_download_spinner
                .set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            re_download_spinner.set_prefix(format!("0x{:02X}", *step));
            re_download_spinner.set_message(format!(
                "Checking whether a re-download is necessary at address 0x{:08X}...",
                file.address
            ));
            *step += 1;
        }

        let response = tool.command(Command::Verify {
            address: file.address,
            len: file.file.metadata()?.len() as u32,
            crc: file.crc32,
        })?;

        if response == Response::Ok {
            if !tool.base().quiet {
                re_download_spinner.finish_with_message("No need to re-download, skip!");
            }
            return Ok(());
        }

        if !tool.base().quiet {
            re_download_spinner.finish_with_message("Need to re-download");
            download_bar.set_style(download_bar_template);
            download_bar.set_message(format!("Download at 0x{:08X}...", file.address));
            download_bar.set_prefix(format!("0x{:02X}", *step));
            *step += 1;
        }

        let res = tool.command(Command::WriteAndErase {
            address: file.address,
            len: file.file.metadata()?.len() as u32,
        })?;
        if res != Response::RxWait {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Write flash failed",
            ));
        }

        let mut buffer = vec![0u8; 128 * 1024];
        let mut reader = BufReader::new(&file.file);

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let res = tool.send_data(&buffer[..bytes_read])?;
            if res == Response::RxWait {
                if !tool.base().quiet {
                    download_bar.inc(bytes_read as u64);
                }
                continue;
            } else if res != Response::Ok {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Write flash failed",
                ));
            }
        }

        if !tool.base().quiet {
            download_bar.finish_with_message("Download success!");
        }

        // verify
        if verify {
            Self::verify(
                tool,
                file.address,
                file.file.metadata()?.len() as u32,
                file.crc32,
            )?;
        }

        Ok(())
    }

    /// 写入单个文件到Flash（全擦除模式）
    pub fn write_file_full_erase<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        step: &mut i32,
        verify: bool,
        packet_size: usize,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let download_bar = ProgressBar::new(file.file.metadata()?.len());
        let download_bar_template = ProgressStyle::default_bar()
            .template("[{prefix}] {msg} {wide_bar} {bytes_per_sec} {percent_precise}%")
            .unwrap()
            .progress_chars("=>-");

        let mut buffer = vec![0u8; packet_size];
        let mut reader = BufReader::new(&file.file);

        if !tool.base().quiet {
            download_bar.set_style(download_bar_template);
            download_bar.set_message(format!("Download at 0x{:08X}...", file.address));
            download_bar.set_prefix(format!("0x{:02X}", *step));
            *step += 1;
        }

        let mut address = file.address;
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            tool.port().write_all(
                Command::Write {
                    address: address,
                    len: bytes_read as u32,
                }
                .to_string()
                .as_bytes(),
            )?;
            tool.port().flush()?;
            let res = tool.send_data(&buffer[..bytes_read])?;
            if res != Response::Ok {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Write flash failed",
                ));
            }
            address += bytes_read as u32;
            if !tool.base().quiet {
                download_bar.inc(bytes_read as u64);
            }
        }
        if !tool.base().quiet {
            download_bar.finish_with_message("Download success!");
        }

        // verify
        if verify {
            Self::verify(
                tool,
                file.address,
                file.file.metadata()?.len() as u32,
                file.crc32,
            )?;
        }

        Ok(())
    }

    // === 异步版本的方法 ===

    /// 擦除所有Flash区域（异步版本）
    pub async fn erase_all_async<T>(
        tool: &mut T,
        write_flash_files: &[WriteFlashFile],
        progress_callback: Option<&ProgressCallback>,
        step: u32,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step,
                total_steps: None,
                current_file: None,
                bytes_processed: 0,
                total_bytes: None,
                message: "Erasing all flash regions...".to_string(),
            });
        }

        let mut erase_address: Vec<u32> = Vec::new();
        for f in write_flash_files.iter() {
            let address = f.address & 0xFF00_0000;
            // 如果ERASE_ADDRESS中的地址已经被擦除过，则跳过
            if erase_address.contains(&address) {
                continue;
            }
            tool.command(Command::EraseAll { address: f.address })?;
            erase_address.push(address);
        }

        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step: step + 1,
                total_steps: None,
                current_file: None,
                bytes_processed: 0,
                total_bytes: None,
                message: "All flash regions erased".to_string(),
            });
        }

        Ok(())
    }

    /// 验证数据（异步版本）
    pub async fn verify_async<T>(
        tool: &mut T, 
        address: u32, 
        len: u32, 
        crc: u32,
        progress_callback: Option<&ProgressCallback>,
        step: u32,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step,
                total_steps: None,
                current_file: None,
                bytes_processed: 0,
                total_bytes: Some(len as u64),
                message: "Verifying data...".to_string(),
            });
        }

        let response = tool.command(Command::Verify { address, len, crc })?;
        if response != Response::Ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Verify failed",
            ));
        }

        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step: step + 1,
                total_steps: None,
                current_file: None,
                bytes_processed: len as u64,
                total_bytes: Some(len as u64),
                message: "Verify success!".to_string(),
            });
        }

        Ok(())
    }

    /// 写入单个文件到Flash（非全擦除模式，异步版本）
    pub async fn write_file_incremental_async<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        step: u32,
        verify: bool,
        progress_callback: Option<&ProgressCallback>,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step,
                total_steps: None,
                current_file: None,
                bytes_processed: 0,
                total_bytes: Some(file.file.metadata()?.len()),
                message: format!("Checking whether a re-download is necessary at address 0x{:08X}...", file.address),
            });
        }

        let response = tool.command(Command::Verify {
            address: file.address,
            len: file.file.metadata()?.len() as u32,
            crc: file.crc32,
        })?;

        if response == Response::Ok {
            if let Some(callback) = progress_callback {
                callback(ProgressInfo {
                    step: step + 1,
                    total_steps: None,
                    current_file: None,
                    bytes_processed: file.file.metadata()?.len(),
                    total_bytes: Some(file.file.metadata()?.len()),
                    message: format!("File at 0x{:08X} is up to date, skipping download", file.address),
                });
            }
            return Ok(());
        }

        // 需要重新下载，先擦除区域
        let file_size = file.file.metadata()?.len() as u32;
        tool.command(Command::Erase {
            address: file.address,
            len: file_size,
        })?;

        // 写入文件数据
        Self::write_file_data_async(tool, file, step + 1, progress_callback).await?;

        if verify {
            Self::verify_async(
                tool,
                file.address,
                file.file.metadata()?.len() as u32,
                file.crc32,
                progress_callback,
                step + 2,
            ).await?;
        }

        Ok(())
    }

    /// 写入单个文件到Flash（全擦除模式，异步版本）
    pub async fn write_file_full_erase_async<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        step: u32,
        verify: bool,
        _packet_size: usize,
        progress_callback: Option<&ProgressCallback>,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        // 直接写入文件数据（因为已经全擦除了）
        Self::write_file_data_async(tool, file, step, progress_callback).await?;

        if verify {
            Self::verify_async(
                tool,
                file.address,
                file.file.metadata()?.len() as u32,
                file.crc32,
                progress_callback,
                step + 1,
            ).await?;
        }

        Ok(())
    }

    /// 写入文件数据的通用方法（异步版本）
    async fn write_file_data_async<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        step: u32,
        progress_callback: Option<&ProgressCallback>,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let file_size = file.file.metadata()?.len();
        let packet_size = if tool.base().compat { 256 } else { 64 * 1024 };
        let mut bytes_written = 0u64;

        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step,
                total_steps: None,
                current_file: None,
                bytes_processed: 0,
                total_bytes: Some(file_size),
                message: format!("Writing file to address 0x{:08X}", file.address),
            });
        }

        let mut buf_reader = BufReader::new(&file.file);
        let mut buffer = vec![0u8; packet_size];
        let mut current_address = file.address;

        loop {
            let bytes_read = buf_reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk = &buffer[..bytes_read];
            tool.command(Command::Write {
                address: current_address,
                len: bytes_read as u32,
            })?;
            tool.send_data(chunk)?;

            current_address += bytes_read as u32;
            bytes_written += bytes_read as u64;

            if let Some(callback) = progress_callback {
                callback(ProgressInfo {
                    step,
                    total_steps: None,
                    current_file: None,
                    bytes_processed: bytes_written,
                    total_bytes: Some(file_size),
                    message: format!("Writing file to address 0x{:08X} ({}/{})", file.address, bytes_written, file_size),
                });
            }

            // 异步让出控制权
            tokio::task::yield_now().await;
        }

        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step: step + 1,
                total_steps: None,
                current_file: None,
                bytes_processed: file_size,
                total_bytes: Some(file_size),
                message: format!("File written successfully to address 0x{:08X}", file.address),
            });
        }

        Ok(())
    }
}
