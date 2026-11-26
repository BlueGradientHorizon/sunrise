

#[cfg(any(test, feature = "test-support"))]
#[cfg(feature = "client")]
pub mod tests {

use sunrise_client::window::GuiSize;

pub fn gui(framebuffer: [u32; 2], scale: u32) -> GuiSize {
        GuiSize::new(framebuffer, scale, false)
    }
}
