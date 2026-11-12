# Does to a process what the POSIX signal named by $Signal would.
param(
	[Parameter(Mandatory = $true)][ValidateSet('SIGSTOP', 'SIGCONT', 'SIGINT')][string]$Signal,
	[Parameter(Mandatory = $true)][int]$ProcessId
)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class ProcessSignals {
	[DllImport("ntdll.dll")]
	static extern int NtSuspendProcess(IntPtr process);
	[DllImport("ntdll.dll")]
	static extern int NtResumeProcess(IntPtr process);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
	[DllImport("kernel32.dll")]
	static extern bool CloseHandle(IntPtr handle);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern bool FreeConsole();
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern bool AttachConsole(int pid);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern bool GenerateConsoleCtrlEvent(uint ctrlEvent, uint processGroup);

	const uint PROCESS_SUSPEND_RESUME = 0x0800;
	const uint CTRL_C_EVENT = 0;
	// Suspensions nest and SIGSTOPs don't: one SIGCONT has to undo any number.
	const int MAX_NESTED_SUSPENSIONS = 16;

	static IntPtr Open(int pid) {
		IntPtr process = OpenProcess(PROCESS_SUSPEND_RESUME, false, pid);
		if (process == IntPtr.Zero) throw new Win32Exception();
		return process;
	}

	public static void Stop(int pid) {
		IntPtr process = Open(pid);
		try {
			int status = NtSuspendProcess(process);
			if (status != 0) throw new InvalidOperationException("NtSuspendProcess: " + status);
		} finally {
			CloseHandle(process);
		}
	}

	public static void Continue(int pid) {
		IntPtr process = Open(pid);
		try {
			for (int i = 0; i < MAX_NESTED_SUSPENSIONS; i++) {
				int status = NtResumeProcess(process);
				if (status != 0) throw new InvalidOperationException("NtResumeProcess: " + status);
			}
		} finally {
			CloseHandle(process);
		}
	}

	// Ctrl-C only reaches a process through a console it shares, and goes to
	// everything on that console, so this one ignores its own.
	public static void Interrupt(int pid) {
		FreeConsole();
		if (!AttachConsole(pid)) throw new Win32Exception();
		if (!SetConsoleCtrlHandler(IntPtr.Zero, true)) throw new Win32Exception();
		if (!GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)) throw new Win32Exception();
	}
}
'@

switch ($Signal) {
	'SIGSTOP' { [ProcessSignals]::Stop($ProcessId) }
	'SIGCONT' { [ProcessSignals]::Continue($ProcessId) }
	'SIGINT' { [ProcessSignals]::Interrupt($ProcessId) }
}
