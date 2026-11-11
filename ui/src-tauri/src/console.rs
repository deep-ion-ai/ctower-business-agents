//! Console no Windows para os modos de linha de comando (ver `main.rs`).

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
};

/// Anexa ao console do processo pai quando a saída padrão não existe.
///
/// Com a saída redirecionada (pipe ou arquivo, o caso do Claude Code), não faz nada.
pub fn anexar_se_preciso() {
    // SAFETY: chamadas sem ponteiros de entrada; só consultam e anexam o console.
    unsafe {
        let saida = GetStdHandle(STD_OUTPUT_HANDLE);
        if saida.is_null() || saida == INVALID_HANDLE_VALUE {
            // Falha (pai sem console) é aceitável: a saída continua descartada.
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
