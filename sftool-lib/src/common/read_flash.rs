use crate::SifliToolTrait;
use crate::common::ram_command::{Command, RamCommand};
use crate::utils::Utils;
use crate::{ProgressCallback, ProgressInfo};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Seek, Write};
use tempfile::tempfile;

/// 通用的Flash读取文件结构
#[derive(Debug)]
pub struct ReadFlashFile {
    pub file_path: String,
    pub address: u32,
    pub size: u32,
}

/// 通用的Flash读取操作实现
pub struct FlashReader;

impl FlashReader {
    /// 解析读取文件信息 (filename@address:size格式)
    pub fn parse_file_info(file_spec: &str) -> Result<ReadFlashFile, std::io::Error> {
        let Some((file_path, addr_size)) = file_spec.split_once('@') else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid format: {}. Expected: filename@address:size",
                    file_spec
                ),
            ));
        };
        let Some((addr, size)) = addr_size.split_once(':') else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid format: {}. Expected: filename@address:size",
                    file_spec
                ),
            ));
        };

        let address = Utils::str_to_u32(addr)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let size = Utils::str_to_u32(size)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        Ok(ReadFlashFile {
            file_path: file_path.to_string(),
            address,
            size,
        })
    }

    /// 从Flash读取数据的通用实现
    pub fn read_flash_data<T>(
        tool: &mut T,
        address: u32,
        size: u32,
        output_path: &str,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress_bar = ProgressBar::new(size as u64);
        if !tool.base().quiet {
            progress_bar.set_style(
                ProgressStyle::default_bar()
                    .template("[{prefix}] {msg} {wide_bar} {bytes_per_sec} {percent_precise}%")
                    .unwrap()
                    .progress_chars("=>-"),
            );
            progress_bar.set_prefix("READ");
            progress_bar.set_message(format!("Reading from 0x{:08X}...", address));
        }

        // 创建临时文件
        let mut temp_file = tempfile()?;
        let packet_size = 128 * 1024; // 128KB chunks
        let mut remaining = size;
        let mut current_address = address;

        while remaining > 0 {
            let chunk_size = std::cmp::min(remaining, packet_size);

            // 发送读取命令
            let _ = tool.command(Command::Read {
                address: current_address,
                len: chunk_size,
            });

            // 读取数据
            let mut buffer = vec![0u8; chunk_size as usize];
            let mut total_read = 0;
            let start_time = std::time::SystemTime::now();

            while total_read < chunk_size {
                let remaining_in_chunk = chunk_size - total_read;
                let mut chunk_buffer = vec![0u8; remaining_in_chunk as usize];

                match tool.port().read_exact(&mut chunk_buffer) {
                    Ok(_) => {
                        buffer[total_read as usize..(total_read + remaining_in_chunk) as usize]
                            .copy_from_slice(&chunk_buffer);
                        total_read += remaining_in_chunk;
                    }
                    Err(_) => {
                        // 超时检查
                        if start_time.elapsed().unwrap().as_millis() > 10000 {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "Read timeout",
                            ));
                        }
                        continue;
                    }
                }
            }

            // 写入临时文件
            temp_file.write_all(&buffer)?;

            remaining -= chunk_size;
            current_address += chunk_size;

            if !tool.base().quiet {
                progress_bar.inc(chunk_size as u64);
            }
        }

        if !tool.base().quiet {
            progress_bar.finish_with_message("Read complete");
        }

        // 将临时文件内容写入目标文件
        temp_file.seek(std::io::SeekFrom::Start(0))?;
        let mut output_file = File::create(output_path)?;
        std::io::copy(&mut temp_file, &mut output_file)?;
        Ok(())
    }

    /// 从Flash读取数据并保存到文件（异步版本）
    pub async fn read_flash_data_async<T>(
        tool: &mut T,
        address: u32,
        size: u32,
        output_path: &str,
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
                current_file: Some(output_path.to_string()),
                bytes_processed: 0,
                total_bytes: Some(size as u64),
                message: format!("Reading {} bytes from address 0x{:08X}", size, address),
            });
        }

        let mut temp_file = tempfile()?;
        let chunk_size = if tool.base().compat { 256 } else { 64 * 1024 };
        let mut current_address = address;
        let mut remaining = size;
        let mut bytes_read_total = 0u64;

        while remaining > 0 {
            let current_chunk_size = std::cmp::min(remaining, chunk_size);

            // 发送读取命令
            tool.command(Command::Read {
                address: current_address,
                len: current_chunk_size,
            })?;

            let mut buffer = vec![0u8; current_chunk_size as usize];
            let mut total_read = 0u32;
            let start_time = std::time::Instant::now();

            // 读取数据
            while total_read < current_chunk_size {
                let remaining_in_chunk = current_chunk_size - total_read;
                let mut chunk_buffer = vec![0u8; remaining_in_chunk as usize];

                match tool.port().read_exact(&mut chunk_buffer) {
                    Ok(_) => {
                        buffer[total_read as usize..(total_read + remaining_in_chunk) as usize]
                            .copy_from_slice(&chunk_buffer);
                        total_read += remaining_in_chunk;
                    }
                    Err(_) => {
                        // 超时检查
                        if start_time.elapsed().as_millis() > 10000 {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "Read timeout",
                            ));
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        continue;
                    }
                }
            }

            // 写入临时文件
            temp_file.write_all(&buffer)?;

            remaining -= current_chunk_size;
            current_address += current_chunk_size;
            bytes_read_total += current_chunk_size as u64;

            if let Some(callback) = progress_callback {
                callback(ProgressInfo {
                    step,
                    total_steps: None,
                    current_file: Some(output_path.to_string()),
                    bytes_processed: bytes_read_total,
                    total_bytes: Some(size as u64),
                    message: format!("Reading from 0x{:08X} ({}/{})", address, bytes_read_total, size),
                });
            }

            // 异步让出控制权
            tokio::task::yield_now().await;
        }

        if let Some(callback) = progress_callback {
            callback(ProgressInfo {
                step: step + 1,
                total_steps: None,
                current_file: Some(output_path.to_string()),
                bytes_processed: size as u64,
                total_bytes: Some(size as u64),
                message: "Read complete".to_string(),
            });
        }

        // 将临时文件内容写入目标文件
        temp_file.seek(std::io::SeekFrom::Start(0))?;
        let mut output_file = File::create(output_path)?;
        std::io::copy(&mut temp_file, &mut output_file)?;

        Ok(())
    }
}
