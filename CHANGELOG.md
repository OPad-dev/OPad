# Changelog

## [1.0.0-rc.1](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc...v1.0.0-rc.1) (2026-10-24)


### Features

* **app:** a Restart button restarts the pad, and a module change asks for it in red ([454593b](https://github.com/OPad-dev/OPad/commit/454593bafb273ebd571532096432c3d47e56a1f2))
* **desktop:** add Hall Effect calibration page and daemon auto-launch ([1c08dd2](https://github.com/OPad-dev/OPad/commit/1c08dd252d4902cb50899e4576c28186cc709f82))
* **diag:** the debug firmware logs MX taps the pad never sent ([3552df2](https://github.com/OPad-dev/OPad/commit/3552df2fe0ae5ced2dbd28cb4dfeece005eea293))
* **display:** the tap rate reads KPM, and each widget can show it in KPS ([#8](https://github.com/OPad-dev/OPad/issues/8)) ([b955339](https://github.com/OPad-dev/OPad/commit/b9553395d4d424fe74625feea6fb083ca194c16c))
* **gui:** a still key is drawn perfectly still (jitter suppression) ([6027604](https://github.com/OPad-dev/OPad/commit/60276044a5a27c4bbc1e53700b39e28b5806e266))
* **gui:** Diagnostics counts HE phantom presses and quick double presses ([0f3cc04](https://github.com/OPad-dev/OPad/commit/0f3cc04dde23a8b5e2da8646d889a22af8747e24))
* **gui:** separate key settings sit side by side, one column per key ([27ded5d](https://github.com/OPad-dev/OPad/commit/27ded5d8e4eea48e05562628107c01808d557aa5))
* **gui:** the actuation slider stops where the key really starts to press ([44fc49e](https://github.com/OPad-dev/OPad/commit/44fc49ee12fd22b7c1f1621f34ca2e4de63db0fb))
* **gui:** the Keys tab fits the default window, and Save stays in view ([fcd3326](https://github.com/OPad-dev/OPad/commit/fcd3326872c0504c5861fc08959083fb6598aa69))
* **gui:** the live switches glide at the display's frame rate, with stronger rest smoothing ([cd1a543](https://github.com/OPad-dev/OPad/commit/cd1a543b3470a72a7026536f5323d6afeae5150c))
* **gui:** the live switches move smoothly, and a fast press still shows at once ([3b967ad](https://github.com/OPad-dev/OPad/commit/3b967ad13fe52395300e6b2461e77695de045601))
* **gui:** the live view is a to-scale cut-away of a magnetic switch, with a travel gauge ([89b5866](https://github.com/OPad-dev/OPad/commit/89b5866bb97b8ddd7ac6b00268eff96624469cf1))
* **he:** a Keys tab sets actuation and rapid trigger, with a live cut-away of each switch ([3f712a2](https://github.com/OPad-dev/OPad/commit/3f712a2119b193a715bb5b704fcf10b8517dd279))
* **he:** a trace build streams every raw HE conversion, and a replay tool counts false rapid-trigger events ([8f7b794](https://github.com/OPad-dev/OPad/commit/8f7b794a2c96aa5fb9a51c82f1a96077e99cdd3f))
* **he:** an HE debug firmware logs every press with the readings before it ([a2524f4](https://github.com/OPad-dev/OPad/commit/a2524f482ed98bdf10d0b9f0cdfb060a29499e2a))
* **he:** an HE pad notices its module unplugged or swapped for MX, and asks for a restart ([fb266c5](https://github.com/OPad-dev/OPad/commit/fb266c5fa0f2f7e042c8085fb0ee6a6c7bba7dcf))
* **he:** an HE pad reads both keys through ADC1 DMA and the decision engine (experimental, off by default) ([cea0b2a](https://github.com/OPad-dev/OPad/commit/cea0b2a241bd26f8453e39efcffbca88cfbc106f))
* **he:** plain calibration screen texts, and no on-pad cancel or timeout (CAL[#13](https://github.com/OPad-dev/OPad/issues/13)) ([f59049f](https://github.com/OPad-dev/OPad/commit/f59049fc4a51a83a652c1352d1d7e771523a4da8))
* **he:** the Hall Effect decision engine turns raw readings into key presses, host-tested ([d41deaa](https://github.com/OPad-dev/OPad/commit/d41deaa66ac41dc6e667fefec147d0972a5b8a9a))
* **protocol:** add Hall Effect calibration commands and live progress ([b5100ab](https://github.com/OPad-dev/OPad/commit/b5100ab59eaa81748ca0d0edee8f45d09582761a))
* **protocol:** Hall Effect capability flags, settings, and status ([e23a43e](https://github.com/OPad-dev/OPad/commit/e23a43ecee625e4185ec9365a9b8ecb2266ae5ae))
* **release:** apt and dnf repositories, repository upgrades from the app, and AUR opad-bin ([9ed95d5](https://github.com/OPad-dev/OPad/commit/9ed95d5068259228b00995a3690918d24a187159))
* **ui:** the pad plays an animation for the grade on osu!'s results screen ([#3](https://github.com/OPad-dev/OPad/issues/3)) ([cb03a0c](https://github.com/OPad-dev/OPad/commit/cb03a0cfef3c10d5f01c27b9f88495b5f73c2b53))
* **update:** a signed feed per channel, gradual rollout, and Check now ([a083f06](https://github.com/OPad-dev/OPad/commit/a083f06c92cdb94be7097cc0630a2c43c6051b4c))


### Bug Fixes

* **desktop:** calibration is offered only when HelloAck says the pad can calibrate ([f63d0e6](https://github.com/OPad-dev/OPad/commit/f63d0e609337d10da433dc9dc943722a07da2506))
* **desktop:** calibration requests and progress follow the current pad (CAL[#4](https://github.com/OPad-dev/OPad/issues/4)) ([71dda42](https://github.com/OPad-dev/OPad/commit/71dda4222ff02a9c592a38492186c6c7e144c4bc))
* **gui:** an MX pad shows nothing about Hall Effect ([388e6bf](https://github.com/OPad-dev/OPad/commit/388e6bf36a37fad14c060b3aee15a1417b0d7a2b))
* **gui:** each live switch sits centred over its depth and status text ([bd74b8d](https://github.com/OPad-dev/OPad/commit/bd74b8d2381a3c637325ac9673105717ca4678ae))
* **gui:** the switch tester sees every press on Linux, so ghost taps beep ([1354c7a](https://github.com/OPad-dev/OPad/commit/1354c7ae7731eb8ba69d38891ea620f44b45c0c8))
* **he:** a failed calibration on a locked pad goes back to the lock screen ([9364e62](https://github.com/OPad-dev/OPad/commit/9364e6264afd54d38e41d3e6aafc1b9b0f31956e))
* **he:** a low actuation point presses where the app says, and a key can opt out of the noise floor ([24e4982](https://github.com/OPad-dev/OPad/commit/24e498219dff3c449c52fd7dda3f931fd89a3eae))
* **he:** calibration fails when a key was held during rest (CAL[#12](https://github.com/OPad-dev/OPad/issues/12)) ([32938dc](https://github.com/OPad-dev/OPad/commit/32938dc557776051e37740e15cd4d0a67fadd01a))
* **he:** calibration keeps old values until saving succeeds (CAL[#3](https://github.com/OPad-dev/OPad/issues/3)) ([ccd1714](https://github.com/OPad-dev/OPad/commit/ccd1714c66e10e9c2fd176e1486a4cd6acda6e3e))
* **he:** calibration retains every frame in sample order (CAL[#1](https://github.com/OPad-dev/OPad/issues/1)) ([21ca183](https://github.com/OPad-dev/OPad/commit/21ca18343898f390f1f3ab5f92d8afbac1f539ab))
* **he:** calibration waits for reporting and USB callback fences (CAL[#11](https://github.com/OPad-dev/OPad/issues/11)) ([5518820](https://github.com/OPad-dev/OPad/commit/551882011976d9b5ce9920180f70cdf12006cc75))
* **he:** unchanged HE settings are not rewritten, and a half-saved SetConfig says which half failed ([ba464f1](https://github.com/OPad-dev/OPad/commit/ba464f1629e0a3dfed1ffb8dfdde898aa9415031))
* **input:** a module plugged into a pad booted without one turns its keys off within milliseconds ([04ab60b](https://github.com/OPad-dev/OPad/commit/04ab60be0a51edade7c8728f410479cd9bfd1f5c))
* **protocol:** a bottom dead zone of 0 can be set over the wire ([74c7519](https://github.com/OPad-dev/OPad/commit/74c75194981e18b152832a0e6a407ead4cf64e3e))
* **protocol:** do not advertise calibration until an entry point exists ([107835c](https://github.com/OPad-dev/OPad/commit/107835cc30946b22f1f4bbbeb32cffae608ebb2e))
