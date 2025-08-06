use super::SF32LB52Tool;
use crate::{ReadFlashParams, ProgressCallback};
use crate::common::read_flash::FlashReader;
use crate::read_flash::ReadFlashTrait;
use std::future::Future;
use std::pin::Pin;

impl ReadFlashTrait for SF32LB52Tool {
    fn read_flash<'a>(
        &'a mut self, 
        params: &'a ReadFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
        Box::pin(async move {
            let mut current_step = 1u32;
            
            // 处理每个读取文件
            for (i, file) in params.files.iter().enumerate() {
                if let Some(ref callback) = progress_callback {
                    callback(crate::ProgressInfo {
                        step: current_step,
                        total_steps: Some(params.files.len() as u32),
                        current_file: Some(file.file_path.clone()),
                        bytes_processed: 0,
                        total_bytes: Some(file.size as u64),
                        message: format!("Reading file {}/{} from 0x{:08X}", i + 1, params.files.len(), file.address),
                    });
                }
                
                FlashReader::read_flash_data_async(
                    self, 
                    file.address, 
                    file.size, 
                    &file.file_path,
                    progress_callback.as_ref(),
                    current_step
                ).await?;
                current_step += 1;
            }

            Ok(())
        })
    }
}
