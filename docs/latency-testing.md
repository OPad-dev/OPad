# osu!pad Latency Regression Gate & Testing

## 1. Testing Principle (§3, §33)
Input latency is the primary release gate. The keyboard performance must be compared against a pure HID-only baseline.

## 2. Benchmark Stages
- **Stage A: HID-Only Firmware Baseline**:
  GPIO interrupt + TinyUSB HID keyboard only. 1000 Hz polling endpoint. No display rendering, no CDC protocol traffic.
- **Stage B: HID + CDC Protocol**:
  Daemon connected via USB CDC, exchanging periodic heartbeat and status packets. No LCD rendering.
- **Stage C: HID + CDC + Gameplay Display**:
  LCD actively rendering PP, song progress bar, and map press counters at target 5 Hz refresh rate.
- **Stage D: Full Stack Integration**:
  Full system with tosu WebSocket, state transitions, and background synchronization.

## 3. Regression Gate Criteria (§33.3)
1. **Missed Presses**: 0 missed accepted presses during fast alternating streams (>20 keys/sec).
2. **Key Sticking**: 0 stuck key-down or key-up states.
3. **Outliers**: No >1 ms jitter outliers introduced by auxiliary subsystems (LCD DMA, CDC framing, timer ISRs).
4. **Latency Degradation**: p99.9 input-to-HID-submit latency delta relative to baseline must remain below 0.1 ms.
