use ash::vk;

use crate::imp::FenceInner;
use crate::{CommandBuffer, Fence, Queue, SwapchainImage};

impl Queue {
    pub fn present(&self, frame: SwapchainImage) -> Result<(), vk::Result> {
        {
            let device = &frame.swapchain.device;
            let mut state = frame.swapchain.device.state.lock();
            let command_buffer = state.get_pending_command_buffer(&device)?;
            let texture = &frame.swapchain.textures[frame.image_index as usize];
            texture.transition_usage_now(command_buffer, texture.descriptor.usage)?;
            state.submit_pending_commands(&frame.swapchain.device, &self.inner.queue)?;

            // these should always be empty after pending commands were submitted
            debug_assert_eq!(0, state.get_wait_semaphores().len());
        }

        let image_indices = [frame.image_index];
        let swapchains = [frame.swapchain.handle];
        let present_info = vk::PresentInfoKHR::builder()
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            let suboptimal = frame
                .swapchain
                .device
                .raw_ext
                .swapchain
                .queue_present(self.inner.queue.handle, &present_info)?;
            if suboptimal {
                log::warn!("present: suboptimal")
            }
        }

        frame.swapchain.device.tick()
    }

    pub fn submit(&self, command_buffers: &[CommandBuffer]) -> Result<(), vk::Result> {
        let device = &self.inner.device;

        device.tick()?;

        let mut state = self.inner.device.state.lock();

        for command_buffer in command_buffers.iter() {
            let vk_command_buffer = state.get_pending_command_buffer(&device)?;
            command_buffer.inner.record_commands(vk_command_buffer, &mut state)?;
        }

        state.submit_pending_commands(&device, &self.inner.queue)
    }

    pub fn create_fence(&self) -> Result<Fence, vk::Result> {
        let fence = FenceInner::new(self.inner.device.clone())?;
        Ok(fence.into())
    }
}
