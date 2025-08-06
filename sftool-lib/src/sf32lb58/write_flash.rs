use super::SF32LB58Tool;
use crate::{WriteFlashParams, ProgressCallback};
use crate::common::write_flash::FlashWriter;
use crate::write_flash::WriteFlashTrait;
use std::future::Future;
use std::pin::Pin;

impl WriteFlashTrait for SF32LB58Tool {
    fn write_flash<'a>(
        &'a mut self, 
        params: &'a WriteFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
        Box::pin(async move {
            let packet_size = if self.base.compat { 256 } else { 128 * 1024 };
            let mut current_step = 1u32;

            if params.erase_all {
                if let Some(ref callback) = progress_callback {
                    callback(crate::ProgressInfo {
                        step: current_step,
                        total_steps: Some(params.files.len() as u32 + 1),
                        current_file: None,
                        bytes_processed: 0,
                        total_bytes: None,
                        message: "Starting erase all operation".to_string(),
                    });
                }
                current_step += 1;
                
                FlashWriter::erase_all_async(self, &params.files, progress_callback.as_ref(), current_step).await?;
                current_step += 1;
            }

            for (i, file) in params.files.iter().enumerate() {
                if let Some(ref callback) = progress_callback {
                    callback(crate::ProgressInfo {
                        step: current_step,
                        total_steps: Some(params.files.len() as u32 + if params.erase_all { 1 } else { 0 }),
                        current_file: Some(format!("File {}/{}", i + 1, params.files.len())),
                        bytes_processed: 0,
                        total_bytes: None,
                        message: format!("Processing file at address 0x{:08X}", file.address),
                    });
                }

                if !params.erase_all {
                    FlashWriter::write_file_incremental_async(
                        self, 
                        file, 
                        current_step, 
                        params.verify,
                        progress_callback.as_ref()
                    ).await?;
                } else {
                    FlashWriter::write_file_full_erase_async(
                        self,
                        file,
                        current_step,
                        params.verify,
                        packet_size,
                        progress_callback.as_ref()
                    ).await?;
                }
                current_step += 1;
            }
            Ok(())
        })
    }
}
