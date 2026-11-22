//! Sets the Windows stack reservation and embeds resources for `sunrise-official`.
#[path = "../../packaging/windows/exe.rs"]
mod windows_exe;

fn main() {
    windows_exe::resources("sunrise-official");
}
