# Prints the pids of the processes named $Name that match the given filter:
# what pgrep, pkill -f and /proc/<pid>/environ answer on the other hosts.
param(
	[Parameter(Mandatory = $true)][string]$Name,
	[string]$ExecutablePath,
	[string]$CommandLineMarker,
	[string]$EnvMarker
)
$ErrorActionPreference = 'Stop'

$candidates = @(Get-CimInstance Win32_Process -Filter "Name='$Name'")
if ($ExecutablePath) {
	$candidates = @($candidates | Where-Object { $_.ExecutablePath -ieq $ExecutablePath })
}
if ($CommandLineMarker) {
	$candidates = @($candidates | Where-Object {
		$_.CommandLine -and $_.CommandLine.IndexOf($CommandLineMarker, [StringComparison]::OrdinalIgnoreCase) -ge 0
	})
}
if ($candidates.Count -eq 0) { exit 0 }
if (-not $EnvMarker) {
	$candidates | ForEach-Object { $_.ProcessId }
	exit 0
}

# Windows has no API for another process's environment, so it is read from the
# process's memory. The offsets are the x64 PEB and RTL_USER_PROCESS_PARAMETERS.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class ProcessEnvironment {
	[StructLayout(LayoutKind.Sequential)]
	struct BasicInformation {
		public IntPtr Reserved1;
		public IntPtr PebBaseAddress;
		public IntPtr Reserved2;
		public IntPtr Reserved3;
		public IntPtr UniqueProcessId;
		public IntPtr Reserved4;
	}

	[DllImport("ntdll.dll")]
	static extern int NtQueryInformationProcess(IntPtr process, int infoClass, ref BasicInformation info, int size, out int returned);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern bool ReadProcessMemory(IntPtr process, IntPtr address, byte[] buffer, IntPtr size, out IntPtr read);
	[DllImport("kernel32.dll")]
	static extern bool CloseHandle(IntPtr handle);

	const uint QUERY_INFORMATION_AND_VM_READ = 0x0400 | 0x0010;

	static byte[] Read(IntPtr process, IntPtr address, long size) {
		byte[] buffer = new byte[size];
		IntPtr read;
		if (!ReadProcessMemory(process, address, buffer, new IntPtr(size), out read)) return null;
		return buffer;
	}

	static IntPtr ReadPointer(IntPtr process, IntPtr address) {
		byte[] bytes = Read(process, address, IntPtr.Size);
		return bytes == null ? IntPtr.Zero : new IntPtr(BitConverter.ToInt64(bytes, 0));
	}

	public static string Of(int pid) {
		IntPtr process = OpenProcess(QUERY_INFORMATION_AND_VM_READ, false, pid);
		if (process == IntPtr.Zero) return null;
		try {
			BasicInformation info = new BasicInformation();
			int returned;
			int status = NtQueryInformationProcess(process, 0, ref info, Marshal.SizeOf(info), out returned);
			if (status != 0) return null;
			IntPtr parameters = ReadPointer(process, IntPtr.Add(info.PebBaseAddress, 0x20));
			if (parameters == IntPtr.Zero) return null;
			IntPtr environment = ReadPointer(process, IntPtr.Add(parameters, 0x80));
			IntPtr size = ReadPointer(process, IntPtr.Add(parameters, 0x3F0));
			if (environment == IntPtr.Zero || size == IntPtr.Zero) return null;
			byte[] block = Read(process, environment, size.ToInt64());
			return block == null ? null : Encoding.Unicode.GetString(block);
		} finally {
			CloseHandle(process);
		}
	}
}
'@

foreach ($candidate in $candidates) {
	$environment = [ProcessEnvironment]::Of([int]$candidate.ProcessId)
	if ($environment -and $environment.IndexOf($EnvMarker, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
		$candidate.ProcessId
	}
}
