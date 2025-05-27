# Drawing to the Screen
Most likely, you will not be able to read from COM1 on if you run the OS on a real computer, or use a debugger on it. So let's draw to the screen to make sure that our OS works on real machines!

To draw to the screen, we will be writing to a region of memory which is memory mapped to a frame buffer. A frame buffer basically represents the pixels on the screen. You typically put a dot on the screen by writing the pixel's RGB values to the region in the frame buffer corresponding to that pixel. Limine makes it easy for us to get a frame buffer. Let's add the Limine request. Before we add the request, let's move all of the Limine-related stuff to it's own module, `limine_requests.rs`. Then let's create the request:
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static FRAME_BUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();
```
To draw shapes, text, and more, we'll use the `embedded-graphics` crate. Add it to `kernel/Cargo.toml`:
```toml


# Learn more
- https://wiki.osdev.org/Drawing_In_a_Linear_Framebuffer
- https://wiki.osdev.org/GOP
