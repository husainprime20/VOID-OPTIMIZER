# Emulator safety

An Android emulator is not a game process. MSI App Player 5 (BlueStacks 5 under
MSI's badge), LDPlayer, MuMu and Nox all run the game inside a virtual machine
with a hypervisor underneath, a VM service beside it and an ADB bridge attached.
Treating any of those like a game — re-prioritising it, pinning it to a core,
killing it with a temp purge — is how an instance stops booting.

This page is the contract: what VOID will touch, what it refuses, what it can put
back, and what it cannot.

---

## What VOID touches

| Target | Allowed | Notes |
| --- | --- | --- |
| `HD-Player.exe`, `dnplayer.exe`, `MuMuPlayer.exe`, `NemuPlayer.exe`, `memu.exe`, `nox.exe`, `GameLoop.exe`, `LdVBoxHeadless.exe` | priority, affinity, GPU preference | The player window only. |
| `BstkSVC.exe`, `HD-Adb.exe`, VM services, hypervisor hosts | **never** | Re-scheduling the VM service hangs the guest. The old `bluestacks*` prefix match did exactly that. |
| A guest younger than **90 s** | **never** | A VM that is still initialising must not be rearranged. VOID says so instead of guessing. |
| `%TEMP%` while any emulator is running | **refused** | BlueStacks keeps live guest state there. The Free Fire profile no longer contains the temp purge at all. |
| BCD `disabledynamictick`, `useplatformtick`, `tscsyncpolicy` | **refused while an emulator is installed** | These survive a reboot and are known to hang virtual machines. |
| `kernel.vbs` (memory integrity / VBS off) | **refused while an emulator is installed** | It flips the hypervisor at the next boot; an instance built against the old one can come back unbootable. |
| Anything inside the guest, the game files, or the instance disk | **never** | VOID tunes the host. It does not touch Free Fire, its anti-cheat, or a `.vdi`. |

The guard is on by default (`Settings → Emulator guard`, also on the Free Fire
page). Turning it off is a deliberate act and is logged.

## What it can put back

**Repair** (Free Fire page) chases the *process* state: Normal priority class,
every logical core, no forced GPU preference. All three travel through
`ff_priority_reset`, `ff_affinity_reset` and `ff_gpu_pref_reset`, so they also
answer to `void-cli revert`.

**Repair boot** chases the *machine* state — the only part that survives a
restart:

1. `bcdedit /set disabledynamictick default`
2. `bcdedit /set useplatformtick default`
3. `bcdedit /set tscsyncpolicy default`
4. `bcdedit /set hypervisorlaunchtype auto`
5. `DeviceGuard\EnableVirtualizationBasedSecurity = 1`,
   `DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity\Enabled = 1`

Only values that are actually wrong are touched, so pressing it twice is safe.
One restart is required for the BCD side to take hold. The Free Fire page shows
each of these as a row — read it before you press anything, it is the honest
answer to "why will my emulator not start".

## Recovering an instance that already will not boot

1. Install 0.10.1 or later and open **Free Fire**. If a boot row is flagged,
   press **repair boot**, then restart Windows.
2. Nothing else VOID changed survives a restart: priority, affinity and GPU
   preference die with the process, so a fresh launch of the player starts clean.
3. If the instance still will not start, the guest disk itself is damaged.
   Reinstall the emulator engine with MSI's own installer — your apps and saves
   live on the Google account, not in the engine — and recreate the instance.
   VOID cannot repair a guest disk, and will not pretend to.

## What is *not* claimed

* The 90-second figure is a judgement, not a measurement published by Bluestack
  or MSI. It is long enough to cover a cold boot on a spinning disk and short
  enough that nobody notices it on an SSD.
* Priority and affinity are read back only on the platforms where that is cheap;
  the UI reports what VOID set, and the Repair path re-asserts it.
* The affinity mask keeps one logical CPU of each pair. On a hybrid CPU (P/E
  cores) that assumption is a simplification; it is not a performance guarantee.
* Nothing here prevents the emulator itself from being broken by a Windows
  update, a driver, or a corrupted instance. It only guarantees VOID is not the
  cause.
