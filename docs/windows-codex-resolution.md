# Windows Codex resolution

On npm-based Windows installs, `where codex` can return both an extensionless POSIX shim and `codex.cmd`. The extensionless shim is not a Win32 executable and produces OS error 193 when passed directly to `CreateProcess`.

BurnCloud Graphs therefore prefers Windows-native executable candidates in this order:

1. `.exe`
2. `.com`
3. `.cmd`
4. `.bat`

Only if none of those exist does it fall back to the first `where.exe` result. `.cmd` and `.bat` programs are launched through `cmd.exe /D /S /C` while preserving stdin for coding-agent prompts.
