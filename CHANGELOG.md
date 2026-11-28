# Changelog

## [1.0.0-rc.9](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.8...v1.0.0-rc.9) (2026-11-28)


### Features

* **firmware:** an update comes over the pad's own USB connection and its animation carries on across the restart ([8de194c](https://github.com/OPad-dev/OPad/commit/8de194cd62a2f66928810d031209dcd5f00095f6))
* **firmware:** the flash after the app slots holds a coredump and two asset packs ([057183b](https://github.com/OPad-dev/OPad/commit/057183be8d7e006427c159524499b8b8cea236ff))
* **ui:** every boot plays the boot animation, and an update starts at once and survives a lost piece ([9694477](https://github.com/OPad-dev/OPad/commit/969447741a028ebbd473bf884ebd8072e8512360))
* **ui:** the pad plays its "updating" animation when it is asked to be flashed ([8353b32](https://github.com/OPad-dev/OPad/commit/8353b32079abc6f19a0f09e95f8801b2a0022a37))

## [1.0.0-rc.8](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.7...v1.0.0-rc.8) (2026-11-27)


### Bug Fixes

* **windows:** uninstalling leaves no daemon behind: the app waits before starting one again, and is stopped first ([a0a70fd](https://github.com/OPad-dev/OPad/commit/a0a70fd7976b9d56fc33672730af5a3abfa35c17))

## [1.0.0-rc.7](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.6...v1.0.0-rc.7) (2026-11-27)


### Features

* **case:** V4 puts the V3 case's inside in a faceted outside ([2abf596](https://github.com/OPad-dev/OPad/commit/2abf596d80548ad30b207f5c142f5123f6e3e72d))
* **config:** a setting from newer firmware is not overwritten by an older app ([0cd9f32](https://github.com/OPad-dev/OPad/commit/0cd9f32ec11f75fdc687c399b53dfdd600c624c9))
* **designer:** both screens apply at once, edits undo, and reset really returns to the pad's own layout ([0bd91e1](https://github.com/OPad-dev/OPad/commit/0bd91e1228061aa3e0cd9237eb855a176b98fab1))
* **gui:** results show as notices that go away by themselves ([01c9e3d](https://github.com/OPad-dev/OPad/commit/01c9e3dad21eb2300174549aba3baa9427997a1a))
* **gui:** Stats names each row by its real key and holds the key speed detail ([a961191](https://github.com/OPad-dev/OPad/commit/a96119147f39b4968e31697fecd2798d1c31856d))
* **gui:** the app keeps one connection to the service and is sent what changed ([0447047](https://github.com/OPad-dev/OPad/commit/0447047c1bebc4813ce1b7319e1df3ca668e60b7))
* **gui:** the menus are regrouped into Play, Your pad and App ([bfc19b2](https://github.com/OPad-dev/OPad/commit/bfc19b2eb949f8573516412ea1c73d410438dd0c))
* **gui:** the window has rounded corners, and its top bar is part of the page ([7bf724a](https://github.com/OPad-dev/OPad/commit/7bf724a09ac24cd9f6aa974631da70ebfdaeef55))
* **ipc:** a client can stay connected and be sent answers as they change ([fb75e78](https://github.com/OPad-dev/OPad/commit/fb75e788d28e32efbeb59ee9c7b5d65580142e35))
* **opadctl:** commands are grouped, results come as JSON on request, and each kind of failure has its exit code ([682a0e7](https://github.com/OPad-dev/OPad/commit/682a0e7b7e1c8c7e6f17efcdd0e4e899ce68c4cb))
* **opadctl:** flash and recover stay at the top level, and the docs use those names ([63f1476](https://github.com/OPad-dev/OPad/commit/63f147671be36346639a8f4257f1934e61cfdf90))
* **release:** an app release carries only files a user downloads, named for their system ([90f2b7a](https://github.com/OPad-dev/OPad/commit/90f2b7a89fc6340af6b78dc0d0db4761f662cc99))
* **update:** a pad speaking another protocol can still get its firmware update ([f11a599](https://github.com/OPad-dev/OPad/commit/f11a5998f2cd28ed158705b639a230724f69c3c9))
* **usb:** the pad enumerates as 303a:8415, OPad's own PID from Espressif (0.9) ([b6c47ad](https://github.com/OPad-dev/OPad/commit/b6c47adf65853fcc0ef226292af42598af953cf9))


### Bug Fixes

* **config:** settings are reported as saved only once the pad has confirmed them ([940a1aa](https://github.com/OPad-dev/OPad/commit/940a1aac3908a692c732dae6c5ab899de3c3066b))
* **daemon:** a counter reset during a sync is no longer undone by it ([fbf1a9f](https://github.com/OPad-dev/OPad/commit/fbf1a9fbf57bb83763e4d61abdce2d2ef7107b10))
* **daemon:** only one thing flashes the pad at a time ([ec4cb15](https://github.com/OPad-dev/OPad/commit/ec4cb157deaa42520c502ae7e5623cc7665847d6))
* **daemon:** two requests at once can no longer stop the daemon for good ([2f38dc7](https://github.com/OPad-dev/OPad/commit/2f38dc773764d557f901125d3fc9a21d06dc2c22))
* **flash:** opadctl flash with the running slot unknown boots what it wrote ([f645d41](https://github.com/OPad-dev/OPad/commit/f645d41e26557d3735ceef1e01232feb447cadca))
* **gui:** each action reports its own result, and Keys stays unsaved until the pad confirms ([4b8e47a](https://github.com/OPad-dev/OPad/commit/4b8e47aed1cd95872fa8c0a87de29c8e0ce16fcd))
* **gui:** nothing behind an open dialog can be clicked ([46537d7](https://github.com/OPad-dev/OPad/commit/46537d7116ba466e174738e2384d081d61e4e1df))
* **gui:** two launches at once no longer both become the instance on Linux ([ce32ed3](https://github.com/OPad-dev/OPad/commit/ce32ed3cd1af70d3468325c97fd6f316a6cfcc8f))
* **recover:** opadctl recover never writes a board that was already in download mode ([17c0320](https://github.com/OPad-dev/OPad/commit/17c03208cb4662aa2916af865f04f41875d86fc7))
* **update:** a failed automatic install waits a day, and the feed survives a restart ([c62a2cc](https://github.com/OPad-dev/OPad/commit/c62a2ccc060e9863503a7bb221af0302a52b5f31))
* **update:** the root update helper reads only the calling user's own files ([bff49d0](https://github.com/OPad-dev/OPad/commit/bff49d0d06bfea4006f7698794305a701b454560))

## [1.0.0-rc.6](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.5...v1.0.0-rc.6) (2026-11-23)


### Features

* **cli:** opadctl update shows, checks for and installs app updates ([c8acc14](https://github.com/OPad-dev/OPad/commit/c8acc14cedc067a165f22230421da309660da40b))
* **diag:** the pad reports its free internal RAM, and the UI test its frame times ([632e839](https://github.com/OPad-dev/OPad/commit/632e8399cb455790cd921dde4cc15d57d582abb8))
* **gui:** --page updates opens Settings on its Updates tab ([c6a71c7](https://github.com/OPad-dev/OPad/commit/c6a71c726629f67978d111a657e79686e2427894))
* **update:** every install updates itself silently; the new version runs from the next start ([3314389](https://github.com/OPad-dev/OPad/commit/3314389457d6abb73bcec6c2d992357851e4641e))
* **update:** the pad's firmware updates itself once the pad has gone 30 minutes unused ([a215e7a](https://github.com/OPad-dev/OPad/commit/a215e7a1766f9eccdbcb9d3ac7be079472739e0a))


### Bug Fixes

* **ci:** the update helper's signed test feed keeps its bytes on Windows checkouts ([a50b6ba](https://github.com/OPad-dev/OPad/commit/a50b6ba0812341d703842f9a5f013e8b3be9d9b2))
* **config:** the app no longer offers GPIO 8 as a key pin, which the firmware rejects (F7) ([03b64e6](https://github.com/OPad-dev/OPad/commit/03b64e642168ccb457e61bcd0f5a0e0cde446796))
* **gui:** the incompatible-pad banner names the pad protocol required, not the IPC version (F5) ([1b03691](https://github.com/OPad-dev/OPad/commit/1b03691db7c1467d1cd385dfd424befe7626fdcf))
* **gui:** the Updates tab knows tosu's version and packaging from the start, and says plainly when no pad is connected ([eadf0c6](https://github.com/OPad-dev/OPad/commit/eadf0c608b782cfcd329d8d575cb67736eca1154))
* **update:** a pad on ota_1 can take the next firmware update from the app (F3) ([7a22b53](https://github.com/OPad-dev/OPad/commit/7a22b5352e1bbc270e18b6a78e9f5d5ed4deaaef))
* **update:** a pad whose screen fell asleep before the daemon connected can update itself ([08b8113](https://github.com/OPad-dev/OPad/commit/08b8113e2cef60b61dd7d77a85ff09adf43150d2))
* **update:** the passwordless helper trusts only the feed key, never the public test key ([b01627a](https://github.com/OPad-dev/OPad/commit/b01627a39a9315f671346277179b5be9b23f98e9))
* **update:** the Windows installer is started outside the daemon's process tree ([852da31](https://github.com/OPad-dev/OPad/commit/852da31ea508b38e27399f8542f6d321e3beefc2))

## [1.0.0-rc.5](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.4...v1.0.0-rc.5) (2026-11-22)


### Bug Fixes

* **lab:** each file goes into the VM as its own path ([b1e79ae](https://github.com/OPad-dev/OPad/commit/b1e79aefe215741c58eff863d77bbdbbf81ed358))
* **lab:** the Linux VMs get their script with LF whatever the checkout has ([1b20a17](https://github.com/OPad-dev/OPad/commit/1b20a171f02a93c2b5661612a8939a61c3bdcb29))
* **lab:** the Windows install phase waits for the setup, not the daemon it starts ([711a96e](https://github.com/OPad-dev/OPad/commit/711a96e4f91d9560aa857d921c0672e4ca64d327))
* **packaging:** opad-bin installs the release's binaries unstripped ([17322a9](https://github.com/OPad-dev/OPad/commit/17322a900498fe326671167e960846fdac79f16c))

## [1.0.0-rc.4](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.3...v1.0.0-rc.4) (2026-11-22)


### Bug Fixes

* **aur:** opad-bin installs: the Vulkan loader is vulkan-icd-loader on Arch ([895da4f](https://github.com/OPad-dev/OPad/commit/895da4f9da8d91a77834ce44949857435b5cce29))
* **deps:** url 2.5.9 (RUSTSEC-2026-0592) ([ca0dd5d](https://github.com/OPad-dev/OPad/commit/ca0dd5dca9f7d064c3546fa890ed233c8496e360))
* **lab:** shell scripts reach the Linux VMs with LF line endings ([86f7bf0](https://github.com/OPad-dev/OPad/commit/86f7bf0dbb5e9aa8b681f75230f3e38bc3f8312c))
* **packaging:** uninstalling OPad leaves nothing of it behind ([3773c5a](https://github.com/OPad-dev/OPad/commit/3773c5a00bdf9c5f8ae2ecf3e753075de25cde36))
* **windows:** the programs start on a clean Windows without the VC++ runtime ([5eb8c03](https://github.com/OPad-dev/OPad/commit/5eb8c03251d16f552ae72ed3889ce1f954c8d2a5))

## [1.0.0-rc.3](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.2...v1.0.0-rc.3) (2026-11-21)


### Continuous Integration

* **release:** the package test's tosu.env lines no longer break its quoting ([1db55d4](https://github.com/OPad-dev/OPad/commit/1db55d49d9107512fba301db1776327a2853c617))

## [1.0.0-rc.2](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc.1...v1.0.0-rc.2) (2026-11-21)


### Bug Fixes

* **tosu:** the bundled tosu never updates itself; OPad's feed updates it ([076fe14](https://github.com/OPad-dev/OPad/commit/076fe143532bed2e4ac80aabc38b7aa7b9a89092))

## [1.0.0-rc.1](https://github.com/OPad-dev/OPad/compare/v1.0.0-rc...v1.0.0-rc.1) (2026-11-21)


### Features

* **app:** a Restart button restarts the pad, and a module change asks for it in red ([454593b](https://github.com/OPad-dev/OPad/commit/454593bafb273ebd571532096432c3d47e56a1f2))
* **backup:** JSON backups carry key 3 and the game keys ([7b883e1](https://github.com/OPad-dev/OPad/commit/7b883e1cc38ac2a6f56453101a768a77baeb8150))
* **case:** V3 case for the three-key modules and the carrier's rear USB-C ([d156d90](https://github.com/OPad-dev/OPad/commit/d156d90b52e75fddfcd183bc4ebbe41a969c76c7))
* **config:** one or two game keys, never three ([c996ee6](https://github.com/OPad-dev/OPad/commit/c996ee6382a991bd6de4ff1921d2b88098b457af))
* **desktop:** add Hall Effect calibration page and daemon auto-launch ([1c08dd2](https://github.com/OPad-dev/OPad/commit/1c08dd252d4902cb50899e4576c28186cc709f82))
* **diag:** a host message the pad cannot decode names the reason, payload and bytes ([5715b9f](https://github.com/OPad-dev/OPad/commit/5715b9fd3749da8fd98dd64a65ddc604278275f7))
* **diag:** the debug firmware logs MX taps the pad never sent ([3552df2](https://github.com/OPad-dev/OPad/commit/3552df2fe0ae5ced2dbd28cb4dfeece005eea293))
* **display:** the default playing layout shows the tap rate in KPS ([1502499](https://github.com/OPad-dev/OPad/commit/15024993a03e300fa28714f00cbe90de55612b3f))
* **display:** the key cards and totals follow the game keys, and Diagnostics tests key 3 ([1f62d72](https://github.com/OPad-dev/OPad/commit/1f62d724d623df94640468f0c873afb3eac43274))
* **display:** the tap rate reads KPM, and each widget can show it in KPS ([#8](https://github.com/OPad-dev/OPad/issues/8)) ([b955339](https://github.com/OPad-dev/OPad/commit/b9553395d4d424fe74625feea6fb083ca194c16c))
* **gui:** a still key is drawn perfectly still (jitter suppression) ([6027604](https://github.com/OPad-dev/OPad/commit/60276044a5a27c4bbc1e53700b39e28b5806e266))
* **gui:** Diagnostics counts HE phantom presses and quick double presses ([0f3cc04](https://github.com/OPad-dev/OPad/commit/0f3cc04dde23a8b5e2da8646d889a22af8747e24))
* **gui:** key pin settings show only on a pad without an input module ([6170048](https://github.com/OPad-dev/OPad/commit/61700487025cc1f364d40f7817659cf5fb0b6dbc))
* **gui:** separate key settings sit side by side, one column per key ([27ded5d](https://github.com/OPad-dev/OPad/commit/27ded5d8e4eea48e05562628107c01808d557aa5))
* **gui:** the actuation slider stops where the key really starts to press ([44fc49e](https://github.com/OPad-dev/OPad/commit/44fc49ee12fd22b7c1f1621f34ca2e4de63db0fb))
* **gui:** the Keys tab fits the default window, and Save stays in view ([fcd3326](https://github.com/OPad-dev/OPad/commit/fcd3326872c0504c5861fc08959083fb6598aa69))
* **gui:** the live switches glide at the display's frame rate, with stronger rest smoothing ([cd1a543](https://github.com/OPad-dev/OPad/commit/cd1a543b3470a72a7026536f5323d6afeae5150c))
* **gui:** the live switches move smoothly, and a fast press still shows at once ([3b967ad](https://github.com/OPad-dev/OPad/commit/3b967ad13fe52395300e6b2461e77695de045601))
* **gui:** the live view is a to-scale cut-away of a magnetic switch, with a travel gauge ([89b5866](https://github.com/OPad-dev/OPad/commit/89b5866bb97b8ddd7ac6b00268eff96624469cf1))
* **hardware:** a board checker rebuilds every trace from the fab files and fails HE V1.0 ([b044bdb](https://github.com/OPad-dev/OPad/commit/b044bdbb4e6cd96ce5a6154c5994aea155225217))
* **hardware:** the USB-C moves to its own board on an 8-way FFC ([f66ed9b](https://github.com/OPad-dev/OPad/commit/f66ed9bf8fa83725170ea4b36c5f62201c10f594))
* **hardware:** V3 carrier, MX and HE boards with one reversible FFC and the USB-C on the carrier ([5a7a7aa](https://github.com/OPad-dev/OPad/commit/5a7a7aa5c2796bca3c0663439f64009b93625e9a))
* **he:** a Keys tab sets actuation and rapid trigger, with a live cut-away of each switch ([3f712a2](https://github.com/OPad-dev/OPad/commit/3f712a2119b193a715bb5b704fcf10b8517dd279))
* **he:** a trace build streams every raw HE conversion, and a replay tool counts false rapid-trigger events ([8f7b794](https://github.com/OPad-dev/OPad/commit/8f7b794a2c96aa5fb9a51c82f1a96077e99cdd3f))
* **he:** an HE debug firmware logs every press with the readings before it ([a2524f4](https://github.com/OPad-dev/OPad/commit/a2524f482ed98bdf10d0b9f0cdfb060a29499e2a))
* **he:** an HE pad notices its module unplugged or swapped for MX, and asks for a restart ([fb266c5](https://github.com/OPad-dev/OPad/commit/fb266c5fa0f2f7e042c8085fb0ee6a6c7bba7dcf))
* **he:** an HE pad reads both keys through ADC1 DMA and the decision engine (experimental, off by default) ([cea0b2a](https://github.com/OPad-dev/OPad/commit/cea0b2a241bd26f8453e39efcffbca88cfbc106f))
* **he:** Hall Effect V3 modules read three keys over SPI, and key 3 is configurable ([9afb870](https://github.com/OPad-dev/OPad/commit/9afb870a62b78a1817f18398a74d310f5694fbc1))
* **he:** plain calibration screen texts, and no on-pad cancel or timeout (CAL[#13](https://github.com/OPad-dev/OPad/issues/13)) ([f59049f](https://github.com/OPad-dev/OPad/commit/f59049fc4a51a83a652c1352d1d7e771523a4da8))
* **he:** the Hall Effect decision engine turns raw readings into key presses, host-tested ([d41deaa](https://github.com/OPad-dev/OPad/commit/d41deaa66ac41dc6e667fefec147d0972a5b8a9a))
* **he:** the V3 module is read at 20 kHz, five times V1's rate ([19b40fb](https://github.com/OPad-dev/OPad/commit/19b40fb40fa5211a13e4679de5a0ac4a7a75c86c))
* **he:** the V3 module's SPI link runs with the ADC's CRC, at 40 MHz, and a frame reads in 14 us ([4b7bc0f](https://github.com/OPad-dev/OPad/commit/4b7bc0f960e4429c58fd391c6f78f9678168d09b))
* **input:** an MX V3 module's key 3 is read on GPIO6 ([e989d7d](https://github.com/OPad-dev/OPad/commit/e989d7d3687e10e9e54fe95188a9415b3d28aa18))
* **protocol:** add Hall Effect calibration commands and live progress ([b5100ab](https://github.com/OPad-dev/OPad/commit/b5100ab59eaa81748ca0d0edee8f45d09582761a))
* **protocol:** Hall Effect capability flags, settings, and status ([e23a43e](https://github.com/OPad-dev/OPad/commit/e23a43ecee625e4185ec9365a9b8ecb2266ae5ae))
* **protocol:** pads are OPAD-&lt;MAC&gt; with their state under opad*, and frames carry a CRC (handoff [#1](https://github.com/OPad-dev/OPad/issues/1)) ([02756fc](https://github.com/OPad-dev/OPad/commit/02756fc8e1b1ed673dfb375e9da6430e37be5f12))
* **release:** apt and dnf repositories, repository upgrades from the app, and AUR opad-bin ([9ed95d5](https://github.com/OPad-dev/OPad/commit/9ed95d5068259228b00995a3690918d24a187159))
* **ui:** the pad plays an animation for the grade on osu!'s results screen ([#3](https://github.com/OPad-dev/OPad/issues/3)) ([cb03a0c](https://github.com/OPad-dev/OPad/commit/cb03a0cfef3c10d5f01c27b9f88495b5f73c2b53))
* **update:** a signed feed per channel, gradual rollout, and Check now ([a083f06](https://github.com/OPad-dev/OPad/commit/a083f06c92cdb94be7097cc0630a2c43c6051b4c))
* **update:** the app reads its feed from updates.opad.dev, and a fork can point it at its own ([a8f6665](https://github.com/OPad-dev/OPad/commit/a8f666505aa1706af0ecf9556080997d84c97202))


### Bug Fixes

* **ci:** CI passes again: patched rustls, tokio, webpki and SQLite, and nanopb pinned ([827ceb0](https://github.com/OPad-dev/OPad/commit/827ceb059d2174ed3000f9ad53a96b41a04db873))
* **daemon:** key 3's count comes back after an erased NVS, and a counter reset zeroes it ([91d077a](https://github.com/OPad-dev/OPad/commit/91d077adc8905923bd316ba7b23fa0293a12be47))
* **desktop:** calibration is offered only when HelloAck says the pad can calibrate ([f63d0e6](https://github.com/OPad-dev/OPad/commit/f63d0e609337d10da433dc9dc943722a07da2506))
* **desktop:** calibration requests and progress follow the current pad (CAL[#4](https://github.com/OPad-dev/OPad/issues/4)) ([71dda42](https://github.com/OPad-dev/OPad/commit/71dda4222ff02a9c592a38492186c6c7e144c4bc))
* **docs:** REUSE passes again; the 2026-09-22 release readiness guide moves to docs/history ([d0b8682](https://github.com/OPad-dev/OPad/commit/d0b8682b99ac423e2ec9db196bbda53d7a36e42e))
* **gui:** an MX pad shows nothing about Hall Effect ([388e6bf](https://github.com/OPad-dev/OPad/commit/388e6bf36a37fad14c060b3aee15a1417b0d7a2b))
* **gui:** each live switch sits centred over its depth and status text ([bd74b8d](https://github.com/OPad-dev/OPad/commit/bd74b8d2381a3c637325ac9673105717ca4678ae))
* **gui:** the switch tester sees every press on Linux, so ghost taps beep ([1354c7a](https://github.com/OPad-dev/OPad/commit/1354c7ae7731eb8ba69d38891ea620f44b45c0c8))
* **gui:** two leftovers of the rename say what they mean again ([5c7ac49](https://github.com/OPad-dev/OPad/commit/5c7ac4928939faf7cad9963963acc9828694d2ce))
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
* **release:** the app release PR brings Cargo.lock to its version ([d2689f8](https://github.com/OPad-dev/OPad/commit/d2689f8666ab5c2150d74ada989a06c2c946bcb9))
