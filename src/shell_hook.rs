use crate::errors::Result;
use crate::shell::ShellType;

pub struct ShellHook;

impl ShellHook {
    pub fn generate_hook(shell: &str) -> Result<String> {
        let shell_type = ShellType::from_name(shell);
        let shell_impl = shell_type.as_shell();
        Ok(shell_impl.hook())
    }
}
