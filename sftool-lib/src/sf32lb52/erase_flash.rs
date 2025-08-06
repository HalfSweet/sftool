use super::SF32LB52Tool;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams, ProgressCallback};
use std::future::Future;
use std::pin::Pin;

impl EraseFlashTrait for SF32LB52Tool {
    fn erase_flash<'a>(
        &'a mut self, 
        params: &'a EraseFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
        Box::pin(async move {
            self.internal_erase_all_async(params.address, progress_callback.as_ref(), 1).await
        })
    }

    fn erase_region<'a>(
        &'a mut self, 
        params: &'a EraseRegionParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
        Box::pin(async move {
            let mut current_step = 1u32;
            
            // 处理每个区域
            for (i, region) in params.regions.iter().enumerate() {
                if let Some(ref callback) = progress_callback {
                    callback(crate::ProgressInfo {
                        step: current_step,
                        total_steps: Some(params.regions.len() as u32),
                        current_file: None,
                        bytes_processed: 0,
                        total_bytes: None,
                        message: format!("Erasing region {}/{} at 0x{:08X}", i + 1, params.regions.len(), region.address),
                    });
                }
                
                self.internal_erase_region_async(
                    region.address, 
                    region.size, 
                    progress_callback.as_ref(),
                    current_step
                ).await?;
                current_step += 1;
            }
            Ok(())
        })
    }
}
