sha: a12b9e140894a83e581beb6f73b84e5fed0aa442
run: 44554135849
ref: refs/heads/arena/4e5e440f-void-optimizer

## selftest
version=0.9.4
boot_ms=7.0
detect_ms=0.2
os=Windows Server 2025 Datacenter 26100.33674
host=runnervmm7ud1
cpu=AMD EPYC 9V74 80-Core Processor
gpu=Microsoft Hyper-V Video
admin=true
simulated=false
tweaks=85
profiles=9
applied=0
cpu_load=0.0
ram_pct=18.3
ram_total_gb=16.0
timer_ms=15.625
power_plan=High performance
emulators_found=0
processes=141
ok=true


## compiler tail
112 | pub fn rose() -> Color32 {
    |        ^^^^

warning: function `veil` is never used
   --> src\theme.rs:148:8
    |
148 | pub fn veil(srgb_alpha: u8) -> Color32 {
    |        ^^^^

warning: function `ease` is never used
   --> src\theme.rs:182:8
    |
182 | pub fn ease(t: f32) -> f32 {
    |        ^^^^

warning: function `split_hairline` is never used
   --> src\theme.rs:351:8
    |
351 | pub fn split_hairline(ui: &mut Ui, inset: f32) {
    |        ^^^^^^^^^^^^^^

warning: function `hot_card` is never used
   --> src\theme.rs:431:8
    |
431 | pub fn hot_card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    |        ^^^^^^^^

warning: function `bands` is never used
   --> src\theme.rs:745:8
    |
745 | pub fn bands(p: &egui::Painter, rect: Rect, now: f64) {
    |        ^^^^^

warning: function `ring` is never used
   --> src\theme.rs:944:8
    |
944 | pub fn ring(ui: &mut Ui, pct: f32, size: f32, caption: &str) {
    |        ^^^^

warning: function `metric` is never used
    --> src\theme.rs:1043:8
     |
1043 | pub fn metric(ui: &mut Ui, label: &str, value: &str, unit: &str) {
     |        ^^^^^^

warning: function `heading` is never used
    --> src\theme.rs:1081:8
     |
1081 | pub fn heading(ui: &mut Ui, text: &str) {
     |        ^^^^^^^

warning: function `sub` is never used
    --> src\theme.rs:1087:8
     |
1087 | pub fn sub(ui: &mut Ui, text: &str) {
     |        ^^^

warning: `void-optimizer` (bin "void-cli") generated 7 warnings
warning: `void-optimizer` (bin "void") generated 21 warnings (1 duplicate)
    Finished `release` profile [optimized] target(s) in 31.21s
