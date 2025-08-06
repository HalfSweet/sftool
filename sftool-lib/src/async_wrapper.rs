use tokio::time::{sleep, Duration, Instant};
use std::io::Error as IoError;

/// 异步包装器，用于将阻塞的串口操作转换为异步
pub struct AsyncSerialWrapper;

impl AsyncSerialWrapper {
    /// 异步读取数据，带超时
    pub async fn read_exact_with_timeout(
        port: &mut Box<dyn serialport::SerialPort>,
        buffer: &mut [u8],
        timeout_ms: u64,
    ) -> Result<(), IoError> {
        let start = Instant::now();
        let mut total_read = 0;
        
        while total_read < buffer.len() {
            if start.elapsed().as_millis() > timeout_ms as u128 {
                return Err(IoError::new(
                    std::io::ErrorKind::TimedOut,
                    "Read operation timed out",
                ));
            }
            
            match port.read(&mut buffer[total_read..]) {
                Ok(bytes_read) => {
                    total_read += bytes_read;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    sleep(Duration::from_millis(1)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        Ok(())
    }
    
    /// 异步写入数据
    pub async fn write_all(
        port: &mut Box<dyn serialport::SerialPort>,
        data: &[u8],
    ) -> Result<(), IoError> {
        // 对于串口写入，我们可以直接调用，因为它通常不会阻塞很久
        port.write_all(data)?;
        port.flush()?;
        Ok(())
    }
    
    /// 异步等待
    pub async fn wait_for_response(
        port: &mut Box<dyn serialport::SerialPort>,
        expected: &[u8],
        timeout_ms: u64,
    ) -> Result<Vec<u8>, IoError> {
        let start = Instant::now();
        let mut buffer = Vec::new();
        
        while start.elapsed().as_millis() < timeout_ms as u128 {
            let mut byte = [0u8; 1];
            match port.read_exact(&mut byte) {
                Ok(_) => {
                    buffer.push(byte[0]);
                    
                    // 检查是否匹配期望的响应
                    if buffer.len() >= expected.len() {
                        let end_slice = &buffer[buffer.len() - expected.len()..];
                        if end_slice == expected {
                            return Ok(buffer);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    sleep(Duration::from_millis(1)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        Err(IoError::new(
            std::io::ErrorKind::TimedOut,
            "Response timeout",
        ))
    }
}
