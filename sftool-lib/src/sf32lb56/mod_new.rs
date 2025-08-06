//! SF32LB56 芯片特定实现模块

pub mod erase_flash;
pub mod ram_command;
pub mod read_flash;
pub mod reset;
pub mod sifli_debug;
pub mod speed;
pub mod write_flash;

use crate::common::sifli_debug::{
    ChipFrameFormat, RecvError, START_WORD, SifliDebug, SifliUartCommand, SifliUartResponse,
    common_debug,
};
use crate::sf32lb56::ram_command::DownloadStub;
use crate::{SifliTool, SifliToolBase, SifliToolTrait};
use serialport::SerialPort;
use std::io::{BufReader, Read};
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;

// Define SF32LB56FrameFormat here to avoid import issues
pub struct SF32LB56FrameFormat;

impl ChipFrameFormat for SF32LB56FrameFormat {
    fn create_header(len: u16) -> Vec<u8> {
        let mut header = vec![];
        header.extend_from_slice(&START_WORD);
        // SF32LB56 uses big-endian for data length
        header.extend_from_slice(&len.to_be_bytes());
        // SF32LB56 adds 4-byte timestamp (fixed to 0)
        header.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        // Channel number fixed to 0x10
        header.push(0x10);
        // CRC fixed to 0x00
        header.push(0x00);
        // reserved field (2 bytes, fixed to 0x00)
        header.extend_from_slice(&[0x00, 0x00]);
        header
    }

    fn parse_frame_header(
        reader: &mut BufReader<Box<dyn Read + Send>>,
    ) -> Result<usize, RecvError> {
        // 读取长度 (2字节) - SF32LB56 uses big-endian
        let mut length_bytes = [0; 2];
        if let Err(e) = reader.read_exact(&mut length_bytes) {
            tracing::error!("Failed to read length bytes: {}", e);
            return Err(RecvError::InvalidHeaderLength);
        }

        let payload_size = u16::from_be_bytes(length_bytes) as usize;

        // 读取时间戳 (4字节) - SF32LB56 specific
        let mut timestamp_bytes = [0; 4];
        if let Err(e) = reader.read_exact(&mut timestamp_bytes) {
            tracing::error!("Failed to read timestamp bytes: {}", e);
            return Err(RecvError::InvalidHeaderChannel);
        }

        // 读取通道和CRC (2字节)
        let mut channel_crc = [0; 2];
        if let Err(e) = reader.read_exact(&mut channel_crc) {
            tracing::error!("Failed to read channel and CRC bytes: {}", e);
            return Err(RecvError::InvalidHeaderChannel);
        }

        // SF32LB56: Read 2-byte reserved field
        let mut reserved_bytes = [0; 2];
        if let Err(e) = reader.read_exact(&mut reserved_bytes) {
            tracing::error!("Failed to read reserved bytes: {}", e);
            return Err(RecvError::ReadError(e));
        }

        Ok(payload_size)
    }

    fn encode_command_data(command: &SifliUartCommand) -> Vec<u8> {
        let mut send_data = vec![];
        match command {
            SifliUartCommand::Enter => {
                let temp = [0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x05, 0x21];
                send_data.extend_from_slice(&temp);
            }
            SifliUartCommand::Exit => {
                let temp = [0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x18, 0x21];
                send_data.extend_from_slice(&temp);
            }
            SifliUartCommand::MEMRead { addr, len } => {
                send_data.push(0x40);
                send_data.push(0x72);
                // SF32LB56 uses big-endian for address and length
                send_data.extend_from_slice(&addr.to_be_bytes());
                send_data.extend_from_slice(&len.to_be_bytes());
            }
            SifliUartCommand::MEMWrite { addr, data } => {
                send_data.push(0x40);
                send_data.push(0x77);
                // SF32LB56 uses big-endian for address and data length
                send_data.extend_from_slice(&addr.to_be_bytes());
                send_data.extend_from_slice(&(data.len() as u16).to_be_bytes());
                for d in data.iter() {
                    send_data.extend_from_slice(&d.to_be_bytes());
                }
            }
        }
        send_data
    }

    fn decode_response_data(data: &[u8]) -> u32 {
        // SF32LB56 uses big-endian
        u32::from_be_bytes([data[0], data[1], data[2], data[3]])
    }

    // SF32LB56 specific address mapping function
    fn map_address(addr: u32) -> u32 {
        let mut l_addr = addr;
        if addr >= 0xE0000000 && addr < 0xF0000000 {
            l_addr = (addr & 0x0fffffff) | 0xF0000000;
            // l_addr = addr
        } else if addr >= 0x00400000 && addr <= 0x0041FFFF {
            // L_RAM
            l_addr += 0x20000000;
        } else if addr >= 0x20C00000 && addr <= 0x20C1FFFF {
            // L_RAM
            l_addr -= 0x00800000;
        } else if addr >= 0x20000000 && addr <= 0x200C7FFF {
            // H_RAM
            l_addr += 0x0A000000;
        } else if addr >= 0x20800000 && addr <= 0x20BFFFFF {
            // H_RAM
            l_addr -= 0x20800000;
        } else if addr >= 0x10000000 && addr <= 0x1FFFFFFF {
            // EXT_MEM
            l_addr += 0x50000000;
        }
        l_addr
    }
}

pub struct SF32LB56Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
}

// SifliDebug trait implementation for SF32LB56Tool
impl SifliDebug for SF32LB56Tool {
    fn debug_command(
        &mut self,
        command: SifliUartCommand,
    ) -> Result<SifliUartResponse, std::io::Error> {
        common_debug::debug_command_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, command)
    }

    fn debug_read_word32(&mut self, addr: u32) -> Result<u32, std::io::Error> {
        common_debug::debug_read_word32_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, addr)
    }

    fn debug_write_word32(&mut self, addr: u32, data: u32) -> Result<(), std::io::Error> {
        common_debug::debug_write_word32_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, addr, data)
    }

    fn debug_write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), std::io::Error> {
        common_debug::debug_write_memory_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, addr, data)
    }

    fn debug_write_core_reg(&mut self, reg: u16, data: u32) -> Result<(), std::io::Error> {
        common_debug::debug_write_core_reg_impl::<SF32LB56Tool, SF32LB56FrameFormat>(
            self, reg, data,
        )
    }

    fn debug_step(&mut self) -> Result<(), std::io::Error> {
        common_debug::debug_step_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self)
    }

    fn debug_run(&mut self) -> Result<(), std::io::Error> {
        common_debug::debug_run_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self)
    }

    fn debug_halt(&mut self) -> Result<(), std::io::Error> {
        common_debug::debug_halt_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self)
    }
}

impl SifliToolTrait for SF32LB56Tool {
    fn port(&mut self) -> &mut Box<dyn SerialPort> {
        &mut self.port
    }

    fn base(&self) -> &SifliToolBase {
        &self.base
    }

    fn set_speed(&mut self, baud: u32) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + '_>> {
        Box::pin(async move {
            use crate::speed::SpeedTrait;
            SpeedTrait::set_speed(self, baud)
        })
    }

    fn soft_reset(&mut self) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + '_>> {
        Box::pin(async move {
            use crate::reset::Reset;
            Reset::soft_reset(self)
        })
    }
}
