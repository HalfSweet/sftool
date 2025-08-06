use super::SF32LB58Tool;
use crate::{ReadFlashParams, ProgressCallback};
use crate::common::read_flash::FlashReader;
use crate::read_flash::ReadFlashTrait;
use std::future::Future;
use std::pin::Pin;

impl ReadFlashTrait for SF32LB58Tool {
    fn read_flash<'a>(
        &'a mut self, 
        params: &'a ReadFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
        Box::pin(async move {
            let total_files = params.files.len();
            
            // 处理每个读取文件
            for (i, file) in params.files.iter().enumerate() {
                if let Some(ref callback) = progress_callback {
                    callback(crate::ProgressInfo {
                        step: (i + 1) as u32,
                        total_steps: Some(total_files as u32),
                        current_file: Some(file.file_path.clone()),
                        bytes_processed: 0,
                        total_bytes: Some(file.size as u64),
                        message: format!("Reading flash at 0x{:08X} to {}", file.address, file.file_path),
                    });
                }
                
                FlashReader::read_flash_data_async(self, file.address, file.size, &file.file_path, progress_callback.as_ref(), (i + 1) as u32).await?;
            }

            Ok(())
        })
    }
}
