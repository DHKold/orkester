# Source - https://superuser.com/a/1596783
# Posted by Hexagon, modified by community. See post 'Timeline' for change history
# Retrieved 2026-03-30, License - CC BY-SA 4.0

Set-StrictMode -version 3.0

# Definition of SetThreadExecutionState from -
#   http://www.pinvoke.net/default.aspx/kernel32/SetThreadExecutionState.html
#   https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setthreadexecutionstate
# "EXECUTION_STATE" changed to "uint", added "public" modifier.
$Signature = @'
[DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
public static extern uint SetThreadExecutionState(uint esFlags);
'@

$ES_AWAYMODE_REQUIRED = 0x00000040L
$ES_CONTINUOUS        = 0x80000000L
$ES_DISPLAY_REQUIRED  = 0x00000002L
$ES_SYSTEM_REQUIRED   = 0x00000001L

# Details on Add-Type in https://docs.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/add-type?view=powershell-7.
$Kernel32 = Add-Type -MemberDefinition $Signature -Name 'Kernel32' -Namespace 'Kernel32' -PassThru

echo "Forcing system to not go to sleep (disable screen saver)."
echo "Close the window to allow sleep again."
$result = $Kernel32::SetThreadExecutionState($ES_CONTINUOUS -bor $ES_DISPLAY_REQUIRED -bor $ES_SYSTEM_REQUIRED)

# Loop until script is forcibly stopped.
While ($true) {
  Start-Sleep (60 * 60 * 24)  # 24 hours.
}
