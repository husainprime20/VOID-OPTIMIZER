# VOID brand

| file | what it is |
| --- | --- |
| `VOID-icon-512.png` | 512×512 PNG with alpha — the asset to upload to Discord |
| `void-icon.png` | 1024×1024 master render |
| `../../assets/void.ico` | multi-size Windows icon, compiled into `VOID.exe` |
| `../../assets/icon64.rgba` | 64×64 raw RGBA, embedded as the live window icon |

## Putting the mark on your Discord presence

The Rich Presence card in the app asks for the asset keys `void_idle` and `void_matchmode`
(and `void_` + the mood for anything else). Discord serves an asset key only if you upload it
to the application first:

1. Open <https://discord.com/developers/applications> and pick **VOID** (id
   `1571961942044713070`).
2. **Rich Presence → Art Assets → Add Image**.
3. Upload `VOID-icon-512.png` and name the asset exactly **`void_idle`**.
4. Upload it a second time (or use *Add Image* again) and name that copy **`void_matchmode`**.
5. Save, restart Discord, restart VOID. The card picks the key up on its next presence push.

The mark is a single ring with one gap and a still point inside: the void that is nearly
closed. It reads at 32 px in the client and at 512 px in the portal.
