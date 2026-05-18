      // Find the Tauri 2 invoke function regardless of which global
      // happens to be present. Tauri 2 with `withGlobalTauri: true`
      // exposes `window.__TAURI__.core.invoke`; the Tauri 1
      // compatibility alias is `window.__TAURI__.tauri.invoke`;
      // some Tauri 2 versions also stash a raw fn at
      // `window.__TAURI_INTERNALS__.invoke`. Use whichever exists.
      function tauriInvoke() {
        if (typeof window === 'undefined') return null;
        const t = window.__TAURI__ || {};
        if (t.core && typeof t.core.invoke === 'function') return t.core.invoke;
        if (t.tauri && typeof t.tauri.invoke === 'function') return t.tauri.invoke;
        if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
          return window.__TAURI_INTERNALS__.invoke;
        }
        return null;
      }

      // Legacy shim — keep working for code paths that still use
      // `window.__TAURI__.tauri.invoke`.
      if (window.__TAURI__ && !window.__TAURI__.tauri && window.__TAURI__.core) {
        window.__TAURI__.tauri = { invoke: window.__TAURI__.core.invoke };
      }

      const entriesEl = document.getElementById('entries');
      const entriesConfigEl = document.getElementById('entriesConfig');
      const targetsEl = document.getElementById('targets');
      const saveConfigBtn = document.getElementById('saveConfig');
      const lastSavedEl = document.getElementById('lastSaved');
      const loadBtn = document.getElementById('load');           // legacy id (removed in current HTML)
      const refreshBtn = document.getElementById('refresh');     // now in Step 2
      const browseIsosBtn = document.getElementById('browseIsosBtn');
      const dropzoneSubEl = document.getElementById('dropzoneSub');
      const topSubtitleEl = document.getElementById('topSubtitle');
      const splash = document.getElementById('splash');
      const app = document.getElementById('app');
      const selectedEl = document.getElementById('selected');
      const confirmInput = document.getElementById('confirmInput');
      const acceptWrite = document.getElementById('acceptWrite');
      const enableWrite = document.getElementById('enableWrite');
      const confirmErase = document.getElementById('confirmErase');
      const scanPathsInput = document.getElementById('scanPaths');
      const scanBtn = document.getElementById('scanBtn');
      const installBtn = document.getElementById('installBtn');
      const progressEl = document.getElementById('progress');
      const bootProgress = document.getElementById('bootProgress');
      const bootProgressFill = document.getElementById('bootProgressFill');
      const entryNote = document.getElementById('entryNote');
      const defaultLabel = document.getElementById('defaultLabel');
      const planEl = document.getElementById('installPlan');
      const planContainer = document.getElementById('planContainer');
      const payloadBadge = document.getElementById('payloadBadge');
      const modeBadge = document.getElementById('modeBadge');
      const configBanner = document.getElementById('configBanner');
      const resetParamsBtn = document.getElementById('resetParams');
      const saveOnInstallOnly = document.getElementById('saveOnInstallOnly');
      const resetModal = document.getElementById('resetModal');
      const cancelReset = document.getElementById('cancelReset');
      const confirmReset = document.getElementById('confirmReset');
      const globalTreeView = document.getElementById('globalTreeView');
      const globalEnableDiskBrowser = document.getElementById('globalEnableDiskBrowser');
      const globalGrubSuperuser = document.getElementById('globalGrubSuperuser');
      const globalGrubPasswordPbkdf2 = document.getElementById('globalGrubPasswordPbkdf2');

      let selectedDisk = null;
      let selectedEspMount = null;
      let selectedDataMount = null;
      let renderedTargets = [];
      let renderedEntries = [];
      let totalIsoBytes = 0;
      let selectedIndex = -1;
      const BOOT_TIMEOUT_SECONDS = 7;
      let bootTimer = null;
      let bootRemaining = 0;

      function formatBytes(bytes) {
        if (!bytes || bytes === 0) return '0 B';
        const units = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(1024));
        const value = (bytes / Math.pow(1024, i)).toFixed(1);
        return `${value} ${units[i]}`;
      }

      function renderEntries(entries) {
        renderedEntries = entries;
        selectedIndex = -1;
        entriesEl.innerHTML = '';
        if (entriesConfigEl) entriesConfigEl.innerHTML = '';
        stopBootTimer();
        if (typeof updateEntryBadge === 'function') updateEntryBadge(entries.length);
        if (!entries.length) {
          // Leave entriesEl empty so the dropzone above (hero) plus
          // the dropzone container CSS take over. The note below
          // names actual host-correct folders.
          if (entryNote) {
            const dirs = (hostInfo.suggested_scan_dirs || []).slice(0, 3);
            entryNote.innerHTML = dirs.length
              ? `No ISOs found in ${dirs.map((d) => `<code>${d}</code>`).join(', ')}. ` +
                `Drop ISO files onto the card above, or click <strong>Browse for ISOs…</strong>.`
              : 'No ISOs found. Drop ISO files onto the card above, ' +
                'or click <strong>Browse for ISOs…</strong>.';
          }
          return;
        }
        entries.forEach((entry) => {
          const el = document.createElement('div');
          el.className = 'disk';
          // Each entry gets a per-ISO verification badge that starts
          // neutral and is updated when verify_iso resolves. The
          // .iso-verify span is scoped to this card via data-path.
          el.innerHTML = `
            <div>
              <strong>${entry.title}</strong>
              <div><small>${entry.subtitle}</small></div>
            </div>
            <div class="iso-badges">
              <span class="badge badge--info iso-verify" data-path="${entry.subtitle || ''}">Checking…</span>
              <span class="pill">${entry.tag}</span>
            </div>
          `;
          el.addEventListener('click', () => selectEntry(entry, el));
          entriesEl.appendChild(el);
          // Fire and forget — the badge updates in place once the
          // hash recompute finishes.
          verifyEntryAsync(entry.subtitle, el.querySelector('.iso-verify'));
        });

        if (entriesConfigEl) {
          renderConfigEntries(entries);
        }

        if (entryNote) entryNote.textContent = 'Entries sourced from ISO files.';
        startBootTimer();
        pushBootConfig();
        updateDefaultLabel();
        document.dispatchEvent(new CustomEvent('raidhos:entries-updated'));
      }

      function renderConfigEntries(entries) {
        entriesConfigEl.innerHTML = '';
        entries.forEach((entry) => {
          const el = document.createElement('div');
          el.className = 'disk';
          const isDefault = entry.path === localStorage.getItem('raidhos_default_entry');
          el.innerHTML = `
            <div>
              <strong>${entry.title}</strong>
              <div><small>${entry.subtitle}</small></div>
              <div><small>Params:</small></div>
              <input class="entry-params" value="${entry.params}" />
              <label class="entry-default">
                <input type="radio" name="defaultEntry" ${isDefault ? 'checked' : ''} />
                Default entry
              </label>
              <div class="actions actions--mt-2">
                <button class="ghost entry-up">Move Up</button>
                <button class="ghost entry-down">Move Down</button>
                <button class="ghost entry-delete">Delete</button>
              </div>
              <details class="entry-advanced">
                <summary>Advanced options</summary>
                <div><small>Initrd (optional)</small></div>
                <input class="entry-initrd" value="${entry.initrd || ''}" />
                <div><small>Kernel args (optional)</small></div>
                <input class="entry-kargs" value="${entry.kargs || ''}" />
                <div><small>Menu class (e.g. linux, windows)</small></div>
                <input class="entry-class" value="${entry.class || ''}" />
                <div><small>Tip / hint shown above the entry</small></div>
                <input class="entry-tip" value="${entry.tip || ''}" />
                <div><small>Persistence backend path (e.g. /persistence/ubuntu.dat)</small></div>
                <input class="entry-persistence" value="${entry.persistence_backend || ''}" />
                <label class="entry-default">
                  <input type="checkbox" class="entry-hidden" ${entry.hidden ? 'checked' : ''} />
                  Hide this entry from the boot menu
                </label>
              </details>
            </div>
            <div class="pill">${entry.tag}</div>
          `;
          const paramsInput = el.querySelector('.entry-params');
          if (paramsInput) {
            paramsInput.addEventListener('input', (ev) => {
              entry.params = ev.target.value;
              persistEntryParams(entry);
              pushBootConfig();
            });
          }
          const defaultInput = el.querySelector('.entry-default input');
          if (defaultInput) {
            defaultInput.addEventListener('change', () => {
              localStorage.setItem('raidhos_default_entry', entry.path);
              pushBootConfig();
              updateDefaultLabel();
            });
          }
          const initrdInput = el.querySelector('.entry-initrd');
          if (initrdInput) {
            initrdInput.addEventListener('input', (ev) => {
              entry.initrd = ev.target.value;
              persistEntryParams(entry);
              pushBootConfig();
            });
          }
          const kargsInput = el.querySelector('.entry-kargs');
          if (kargsInput) {
            kargsInput.addEventListener('input', (ev) => {
              entry.kargs = ev.target.value;
              persistEntryParams(entry);
              pushBootConfig();
            });
          }
          const classInput = el.querySelector('.entry-class');
          if (classInput) {
            classInput.addEventListener('input', (ev) => {
              entry.class = ev.target.value;
              persistEntryParams(entry);
              pushBootConfig();
            });
          }
          const tipInput = el.querySelector('.entry-tip');
          if (tipInput) {
            tipInput.addEventListener('input', (ev) => {
              entry.tip = ev.target.value;
              persistEntryParams(entry);
              pushBootConfig();
            });
          }
          const persistenceInput = el.querySelector('.entry-persistence');
          if (persistenceInput) {
            persistenceInput.addEventListener('input', (ev) => {
              entry.persistence_backend = ev.target.value;
              persistEntryParams(entry);
              pushBootConfig();
            });
          }
          const hiddenInput = el.querySelector('.entry-hidden');
          if (hiddenInput) {
            hiddenInput.addEventListener('change', (ev) => {
              entry.hidden = ev.target.checked;
              persistEntryParams(entry);
              pushBootConfig();
            });
          }
          const upBtn = el.querySelector('.entry-up');
          const downBtn = el.querySelector('.entry-down');
          const delBtn = el.querySelector('.entry-delete');
          if (upBtn) {
            upBtn.addEventListener('click', () => {
              moveEntry(entry.path, -1);
            });
          }
          if (downBtn) {
            downBtn.addEventListener('click', () => {
              moveEntry(entry.path, 1);
            });
          }
          if (delBtn) {
            delBtn.addEventListener('click', () => {
              deleteEntry(entry.path);
            });
          }
          entriesConfigEl.appendChild(el);
        });
      }

      function moveEntry(path, direction) {
        const idx = renderedEntries.findIndex((e) => e.path === path);
        if (idx < 0) return;
        const next = idx + direction;
        if (next < 0 || next >= renderedEntries.length) return;
        const temp = renderedEntries[idx];
        renderedEntries[idx] = renderedEntries[next];
        renderedEntries[next] = temp;
        localStorage.setItem('raidhos_last_isos', JSON.stringify(renderedEntries));
        renderEntries(renderedEntries);
        pushBootConfig();
      }

      function deleteEntry(path) {
        renderedEntries = renderedEntries.filter((e) => e.path !== path);
        localStorage.setItem('raidhos_last_isos', JSON.stringify(renderedEntries));
        renderEntries(renderedEntries);
        pushBootConfig();
      }

      function selectByIndex(index) {
        if (!renderedEntries.length) return;
        const clamped = Math.max(0, Math.min(index, renderedEntries.length - 1));
        const entry = renderedEntries[clamped];
        const el = entriesEl.children[clamped];
        if (!entry || !el) return;
        selectedIndex = clamped;
        selectEntry(entry, el);
      }

      function selectEntry(entry, el) {
        Array.from(entriesEl.children).forEach((d) => d.classList.remove('selected'));
        el.classList.add('selected');
        stopBootTimer();
      }

      // Compute the safety verdict for a candidate target disk:
      //   system  → red, blocked (will not let the user pick it)
      //   small   → amber, allowed but warned
      //   ok      → green, ready
      // The "too small" threshold is the sum of the picked ISOs plus
      // a 1 GiB headroom for ESP + GRUB. If we don't yet know the
      // ISO sizes we just use a conservative 4 GiB minimum.
      function diskVerdict(disk) {
        if (disk.is_system) {
          return { kind: 'system', label: 'System drive · blocked' };
        }
        const headroom = 1024 * 1024 * 1024; // 1 GiB ESP/GRUB
        const fallback = 4 * 1024 * 1024 * 1024; // 4 GiB sane default
        const needed = totalIsoBytes > 0 ? totalIsoBytes + headroom : fallback;
        if (disk.size_bytes && disk.size_bytes < needed) {
          return {
            kind: 'small',
            label: `Too small (${formatBytes(disk.size_bytes)} < ${formatBytes(needed)})`,
          };
        }
        return { kind: 'ok', label: disk.removable ? 'Removable · ready' : 'Fixed · ready' };
      }

      function renderTargets(disks) {
        renderedTargets = disks;
        targetsEl.innerHTML = '';
        if (!disks.length) {
          targetsEl.innerHTML = '<div class="disk">No devices detected.</div>';
          return;
        }
        disks.forEach((disk) => {
          const el = document.createElement('div');
          el.className = 'disk';
          const mounts = disk.mountpoints && disk.mountpoints.length ? ` · ${disk.mountpoints.join(', ')}` : '';
          const verdict = diskVerdict(disk);
          const badgeKind = verdict.kind === 'system'
            ? 'danger'
            : verdict.kind === 'small'
              ? 'warn'
              : 'ok';
          el.innerHTML = `
            <div>
              <strong>${disk.id}</strong>
              <div><small>${disk.model || 'Unknown model'} · ${formatBytes(disk.size_bytes)}${mounts}</small></div>
            </div>
            <span class="badge badge--${badgeKind}">${verdict.label}</span>
          `;
          if (verdict.kind === 'system') {
            el.classList.add('disk--blocked');
            el.setAttribute('aria-disabled', 'true');
            el.title = 'System drive — selecting this would brick your machine. Pick a removable disk.';
          } else {
            el.addEventListener('click', () => selectDisk(disk, el));
          }
          targetsEl.appendChild(el);
        });
      }

      function selectDisk(disk, el) {
        selectedDisk = disk;
        selectedEl.textContent = `${disk.id} (${formatBytes(disk.size_bytes)})`;
        confirmInput.value = '';
        Array.from(targetsEl.children).forEach((d) => d.classList.remove('selected'));
        el.classList.add('selected');
        loadPartitions(disk.id);
        updateInstallState();
      }

      function updateInstallState() {
        const ok = selectedDisk
          && !selectedDisk.is_system
          && confirmInput.value.trim() === selectedDisk.id
          && acceptWrite
          && acceptWrite.checked;
        const allowWriteOk = enableWrite && enableWrite.checked
          ? (confirmErase && confirmErase.value.trim() === 'ERASE')
          : true;
        installBtn.disabled = !ok;
        const destructive = enableWrite && enableWrite.checked;
        if (installBtn) {
          installBtn.textContent = destructive && allowWriteOk
            ? 'Run Install (destructive)'
            : 'Run Dry-Run Install';
        }
        if (destructive && !allowWriteOk) {
          installBtn.disabled = true;
        }
        if (modeBadge) {
          setBadge(
            modeBadge,
            destructive ? 'Destructive · will erase target' : 'Dry-run · no changes made',
            destructive ? 'danger' : 'info',
          );
        }
        updateInstallPlan();
      }

      async function listDisks() {
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const disks = await invoke('list_disks');
          renderTargets(disks);
        } catch (err) {
          targetsEl.innerHTML = `<div class="disk">${String(err)}</div>`;
        }
      }

      async function loadPartitions(device) {
        selectedEspMount = null;
        selectedDataMount = null;
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const parts = await invoke('list_partitions', { device });
          parts.forEach((p) => {
            const mounts = p.mountpoints || [];
            if (!mounts.length) return;
            if (p.label === 'RAIDHOS_EFI' || p.fstype === 'vfat') {
              selectedEspMount = mounts[0];
            }
            if (p.label === 'DATA' || p.fstype === 'exfat') {
              selectedDataMount = mounts[0];
            }
          });
        } catch (_err) {
        }
        updateInstallPlan();
      }

      function startBootTimer() {
        if (!renderedEntries.length) return;
        bootRemaining = BOOT_TIMEOUT_SECONDS;
        updateBootStatus();
        bootTimer = setInterval(() => {
          bootRemaining -= 1;
          updateBootStatus();
          if (bootRemaining <= 0) {
            const defaultIndex = getDefaultIndex();
            selectByIndex(defaultIndex >= 0 ? defaultIndex : 0);
            stopBootTimer();
          }
        }, 1000);
      }

      function stopBootTimer() {
        if (bootTimer) {
          clearInterval(bootTimer);
          bootTimer = null;
        }
        updateBootStatus();
      }

      function updateBootStatus() {
        if (!bootProgress || !bootProgressFill) return;
        if (bootTimer) {
          bootProgress.style.opacity = '1';
          const pct = Math.max(0, Math.min(1, (BOOT_TIMEOUT_SECONDS - bootRemaining) / BOOT_TIMEOUT_SECONDS));
          bootProgressFill.style.width = `${pct * 100}%`;
        } else {
          bootProgress.style.opacity = '0';
          bootProgressFill.style.width = '0%';
        }
      }

      function renderProgress(events) {
        progressEl.innerHTML = '';
        events.forEach((event) => {
          const row = document.createElement('div');
          row.className = 'progress-item';
          const percent = event.percent !== null && event.percent !== undefined ? `${event.percent}%` : '';
          row.innerHTML = `<span>${event.phase}: ${event.message}</span><span>${percent}</span>`;
          progressEl.appendChild(row);
        });
      }

      async function runInstall() {
        if (!selectedDisk) return;
        progressEl.innerHTML = '';
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const isWrite = enableWrite && enableWrite.checked;
          if (isWrite) {
            showBanner('Elevating privileges...', false, true);
            const output = await invoke('install_elevated', {
              device: selectedDisk.id,
              payloadVersion: '1.1.10',
            });
            progressEl.innerHTML = `<div class="progress-item">${String(output)}</div>`;
            await copyIsosToData();
            await writeConfigToTarget();
            await writeGrubCfgToEsp();
            progressEl.innerHTML += '<div class="progress-item">config: written to target</div>';
          } else {
            const events = await invoke('install', {
              device: selectedDisk.id,
              payloadVersion: '1.1.10',
              wipe: true,
              dryRun: true,
              allowWrite: false,
            });
            renderProgress(events);
          }
        } catch (err) {
          progressEl.innerHTML = `<div class="progress-item">${String(err)}</div>`;
        }
      }

      function bootSequence() {
        const minBootMs = 1400;
        const start = Date.now();
        const finish = () => {
          const elapsed = Date.now() - start;
          const wait = Math.max(0, minBootMs - elapsed);
          setTimeout(() => {
            splash.classList.add('hidden');
            app.classList.add('ready');
          }, wait);
        };

        if (window.__TAURI__) {
          finish();
        } else {
          setTimeout(finish, 200);
        }
      }

      confirmInput.addEventListener('input', updateInstallState);
      if (acceptWrite) acceptWrite.addEventListener('change', updateInstallState);
      if (enableWrite) enableWrite.addEventListener('change', updateInstallState);
      if (confirmErase) confirmErase.addEventListener('input', updateInstallState);
      if (loadBtn) loadBtn.addEventListener('click', listDisks);
      if (refreshBtn) refreshBtn.addEventListener('click', listDisks);
      if (browseIsosBtn) {
        browseIsosBtn.addEventListener('click', async () => {
          try {
            // Route through our own Rust command rather than the
            // JS-side dialog global. The plugin doesn't reliably
            // expose itself under `window.__TAURI__.dialog`, so we
            // call `open_iso_picker` which wraps the dialog plugin
            // from the Rust side and returns the paths.
            const inv = tauriInvoke();
            if (!inv) {
              showBanner(
                'Tauri runtime missing — open the app through `cargo run -p raidhos-ui`, not by opening index.html directly.',
                true,
                false,
              );
              return;
            }
            const paths = await inv('open_iso_picker');
            if (Array.isArray(paths) && paths.length) {
              await ingestDroppedIsos(paths);
            }
          } catch (err) {
            showBanner(`Browse failed: ${err}`, true, false);
          }
        });
      }
      installBtn.addEventListener('click', runInstall);
      if (scanBtn) scanBtn.addEventListener('click', loadEntries);
      if (saveConfigBtn) saveConfigBtn.addEventListener('click', async () => {
        if (saveOnInstallOnly && saveOnInstallOnly.checked) {
          showBanner('Save on install only is enabled.', false, false);
          return;
        }
        await copyIsosToData();
        await pushBootConfig();
        await writeConfigToTarget();
        await writeGrubCfgToEsp();
        updateLastSaved();
      });
      if (resetParamsBtn) resetParamsBtn.addEventListener('click', () => {
        if (resetModal) resetModal.classList.add('open');
      });
      if (cancelReset) cancelReset.addEventListener('click', () => {
        if (resetModal) resetModal.classList.remove('open');
      });
      if (confirmReset) confirmReset.addEventListener('click', () => {
        renderedEntries.forEach((entry) => {
          entry.params = 'quiet splash';
          entry.initrd = '';
          entry.kargs = '';
          persistEntryParams(entry);
        });
        renderConfigEntries(renderedEntries);
        pushBootConfig();
        updateLastSaved();
        if (resetModal) resetModal.classList.remove('open');
      });
      document.addEventListener('keydown', (event) => {
        if (!renderedEntries.length) return;
        if (event.key === 'ArrowDown') {
          event.preventDefault();
          selectByIndex(selectedIndex + 1);
        }
        if (event.key === 'ArrowUp') {
          event.preventDefault();
          selectByIndex(selectedIndex - 1);
        }
        if (event.key === 'Enter') {
          selectByIndex(selectedIndex < 0 ? 0 : selectedIndex);
        }
      });

      function getDefaultIndex() {
        const path = localStorage.getItem('raidhos_default_entry');
        if (!path) return -1;
        return renderedEntries.findIndex((entry) => entry.path === path);
      }

      function updateDefaultLabel() {
        if (!defaultLabel) return;
        const path = localStorage.getItem('raidhos_default_entry');
        if (!path) {
          defaultLabel.textContent = '';
          return;
        }
        const entry = renderedEntries.find((e) => e.path === path);
        defaultLabel.textContent = entry ? `Default: ${entry.title}` : '';
      }

      function mapEntryPath(entry) {
        const src = entry.subtitle || entry.path || '';
        const name = src.split('/').pop();
        if (!name) return '/boot/isos/unknown.iso';
        return `/boot/isos/${name}`;
      }

      async function copyIsosToData() {
        if (!selectedDataMount) {
          showBanner('Data partition not mounted. Cannot copy ISOs.', true, false);
          return;
        }
        const sources = renderedEntries.map((entry) => entry.subtitle || entry.path).filter(Boolean);
        if (!sources.length) return;
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          await invoke('copy_isos_to_data', { mountPath: selectedDataMount, sources });
          renderedEntries.forEach((entry) => {
            entry.path = mapEntryPath(entry);
          });
        } catch (_err) {
          showBanner('Failed to copy ISO files to target.', true, false);
        }
      }

      function showBanner(message, isError, isLoading) {
        if (!configBanner) return;
        configBanner.style.display = 'block';
        configBanner.className = isError ? 'banner error' : 'banner';
        if (isLoading) {
          configBanner.innerHTML = `<span class="spinner"></span>${message}`;
        } else {
          configBanner.textContent = message;
        }
      }

      function updateInstallPlan() {
        if (!planEl) return;
        if (planContainer) {
          planContainer.style.display = selectedDisk ? 'grid' : 'none';
        }
        const steps = [];
        steps.push('Validate target and confirmations');
        steps.push('Create GPT and partitions');
        steps.push('Format EFI + data partitions');
        steps.push('Copy payload (if RAIDHOS_PAYLOAD_DIR set)');
        steps.push('Write boot config to target');
        steps.push('Write grub.cfg to ESP');
        if (enableWrite && enableWrite.checked) {
          steps.push('Actual write enabled');
        } else {
          steps.push('Dry-run mode');
        }
        if (selectedDisk && !selectedEspMount) {
          steps.push('ESP not mounted (grub.cfg will be skipped)');
        }
        planEl.innerHTML = steps.map((s) => `<div class="plan-item">${s}</div>`).join('');
      }

      function setBadge(el, text, kind) {
        if (!el) return;
        el.textContent = text;
        el.classList.remove('badge--info', 'badge--ok', 'badge--warn', 'badge--danger');
        el.classList.add(`badge--${kind}`);
      }

      // Pre-flight ISO verification: ask the backend to hash the ISO
      // and compare against any `<iso>.sha256` companion file. Result
      // is rendered as a coloured badge on the entry card — green
      // "Verified", red "Hash mismatch" (corrupt/tampered), or amber
      // "Unverified" when no companion file exists. Tooltip carries
      // the full message.
      async function verifyEntryAsync(path, badgeEl) {
        if (!badgeEl || !path) return;
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const v = await invoke('verify_iso', { path });
          if (!v) return;
          badgeEl.title = v.message || '';
          if (v.kind === 'ok') {
            setBadge(badgeEl, 'Verified', 'ok');
          } else if (v.kind === 'mismatch') {
            setBadge(badgeEl, 'Hash mismatch', 'danger');
          } else {
            setBadge(badgeEl, 'Unverified', 'warn');
          }
        } catch (_err) {
          setBadge(badgeEl, 'Unverified', 'warn');
        }
      }

      async function loadPayloadVersion() {
        const aboutVersionLine = document.getElementById('aboutVersionLine');
        // Don't touch the entry-card status badge — that one
        // reflects "what's in the entries list", not "is the dev
        // payload available". We surface payload status only in
        // the About modal and via a low-noise toast on failure.
        try {
          const invoke = tauriInvoke();
          if (!invoke) throw new Error('Tauri runtime not available');
          const version = await invoke('get_payload_version');
          if (aboutVersionLine) aboutVersionLine.textContent = `Payload version: ${version}`;
        } catch (_err) {
          if (aboutVersionLine) aboutVersionLine.textContent = 'Payload version: missing';
        }
      }

      // Flip the entry-card badge from neutral "Awaiting ISO…" to
      // a positive count once entries land. Called from renderEntries.
      function updateEntryBadge(count) {
        if (!payloadBadge) return;
        if (!count) {
          setBadge(payloadBadge, 'Awaiting ISO…', 'info');
        } else {
          setBadge(payloadBadge, `${count} ISO${count === 1 ? '' : 's'}`, 'ok');
        }
      }

      // Host metadata, fetched once at startup. The fields go into
      // the scan-paths default, the dropzone subtitle, and the
      // entry-list empty-state copy so the UI never tells a macOS
      // user to "drop ISOs in /media".
      let hostInfo = { os: 'unknown', suggested_scan_dirs: ['/media', '/mnt', '/home'] };

      async function loadHostInfo() {
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          hostInfo = await invoke('get_host_info');
        } catch (_err) {
          // Fall through to defaults — non-Tauri preview.
        }
        if (scanPathsInput && !scanPathsInput.value) {
          scanPathsInput.placeholder = hostInfo.suggested_scan_dirs.join(', ');
        }
        if (dropzoneSubEl) {
          const first = hostInfo.suggested_scan_dirs[0] || '~/Downloads';
          dropzoneSubEl.textContent = `or click below — RaidhOS also scans ${first}`;
        }
        if (topSubtitleEl) {
          const osLabel = {
            macos: 'macOS',
            linux: 'Linux',
            windows: 'Windows',
          }[hostInfo.os] || 'this host';
          topSubtitleEl.textContent =
            `Build a multi-ISO bootable USB on ${osLabel}.`;
        }
        // About modal: fill in host line once we know what the OS is.
        const aboutHostLine = document.getElementById('aboutHostLine');
        if (aboutHostLine) {
          aboutHostLine.textContent = `Host: ${hostInfo.os}`;
        }
        // Re-render any empty entry list so the host-aware "scans
        // ~/Downloads" copy replaces the placeholder defaults.
        if (!renderedEntries.length) renderEntries([]);
      }

      function parseScanDirs() {
        const raw = (scanPathsInput && scanPathsInput.value) ? scanPathsInput.value : '';
        if (!raw.trim()) return hostInfo.suggested_scan_dirs;
        return raw
          .split(',')
          .map((s) => s.trim())
          .filter((s) => s.length > 0);
      }

      async function loadEntries() {
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const dirs = parseScanDirs();
          const isos = await invoke('scan_isos', { dirs });
          totalIsoBytes = isos.reduce((acc, iso) => acc + (iso.size_bytes || 0), 0);
          const entries = isos.map((iso) => ({
            title: iso.title,
            subtitle: iso.path,
            tag: 'ISO',
            params: iso.params || 'quiet splash',
            initrd: '',
            kargs: '',
          }));
          localStorage.setItem('raidhos_last_isos', JSON.stringify(entries));
          hydrateEntryParams(entries);
          renderEntries(entries);
          // Re-render the disk targets so the new total flows into
          // the per-disk "Too small" warnings.
          if (renderedTargets.length) renderTargets(renderedTargets);
        } catch (err) {
          renderEntries([]);
        }
      }

      function loadCachedEntries() {
        const raw = localStorage.getItem('raidhos_last_isos');
        if (!raw) return;
        try {
          const cached = JSON.parse(raw);
          if (Array.isArray(cached) && cached.length) {
            hydrateEntryParams(cached);
            renderEntries(cached);
          }
        } catch (_err) {
        }
      }

      function restoreState() {
        if (scanPathsInput) {
          scanPathsInput.value = localStorage.getItem('raidhos_scan_paths') || '';
        }
        if (acceptWrite) {
          acceptWrite.checked = localStorage.getItem('raidhos_accept_write') === 'true';
        }
        if (enableWrite) {
          enableWrite.checked = localStorage.getItem('raidhos_enable_write') === 'true';
        }
        if (saveOnInstallOnly) {
          saveOnInstallOnly.checked = localStorage.getItem('raidhos_save_on_install_only') === 'true';
        }
        if (globalTreeView) {
          globalTreeView.checked = localStorage.getItem('raidhos_tree_view') === 'true';
        }
        if (globalEnableDiskBrowser) {
          globalEnableDiskBrowser.checked =
            localStorage.getItem('raidhos_enable_disk_browser') === 'true';
        }
        if (globalGrubSuperuser) {
          globalGrubSuperuser.value = localStorage.getItem('raidhos_grub_superuser') || '';
        }
        // grub_password_pbkdf2 is intentionally NOT restored from
        // localStorage — even though it's a PBKDF2 hash, the hash
        // enables offline brute-force against the password, and
        // localStorage is reachable from any same-origin XSS.
        // Source of truth lives in boot.json on disk via the
        // Tauri save_boot_config command; the user re-pastes
        // the hash into the field once per session if they want
        // to change it.
      }

      function entryKey(entry) {
        return `raidhos_entry_${entry.path}`;
      }

      function persistEntryParams(entry) {
        if (!entry || !entry.path) return;
        const payload = {
          params: entry.params || '',
          initrd: entry.initrd || '',
          kargs: entry.kargs || '',
          class: entry.class || '',
          tip: entry.tip || '',
          persistence_backend: entry.persistence_backend || '',
          hidden: !!entry.hidden,
        };
        localStorage.setItem(entryKey(entry), JSON.stringify(payload));
      }

      function hydrateEntryParams(entries) {
        entries.forEach((entry) => {
          const raw = localStorage.getItem(entryKey(entry));
          if (!raw) return;
          try {
            const parsed = JSON.parse(raw);
            entry.params = parsed.params || entry.params;
            entry.initrd = parsed.initrd || entry.initrd;
            entry.kargs = parsed.kargs || entry.kargs;
            entry.class = parsed.class || entry.class || '';
            entry.tip = parsed.tip || entry.tip || '';
            entry.persistence_backend =
              parsed.persistence_backend || entry.persistence_backend || '';
            entry.hidden = !!parsed.hidden;
          } catch (_err) {
          }
        });
      }

      async function pushBootConfig() {
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const defaultEntry = localStorage.getItem('raidhos_default_entry');
          const payload = {
            default_entry: defaultEntry || null,
            entries: renderedEntries.map((entry) => ({
              title: entry.title,
              path: entry.path || mapEntryPath(entry),
              params: entry.params || '',
              initrd: entry.initrd || '',
              kargs: entry.kargs || '',
              class: entry.class || '',
              tip: entry.tip || '',
              hidden: !!entry.hidden,
              persistence_backend: entry.persistence_backend || '',
            })),
            tree_view: !!(globalTreeView && globalTreeView.checked),
            enable_disk_browser:
              !!(globalEnableDiskBrowser && globalEnableDiskBrowser.checked),
            grub_superuser:
              (globalGrubSuperuser && globalGrubSuperuser.value) || '',
            grub_password_pbkdf2:
              (globalGrubPasswordPbkdf2 && globalGrubPasswordPbkdf2.value) || '',
          };
          await invoke('save_boot_config', { config: payload });
        } catch (_err) {
        }
      }

      async function writeConfigToTarget() {
        if (!selectedDisk) return;
        const mount = selectedDataMount || (selectedDisk.mountpoints && selectedDisk.mountpoints.length ? selectedDisk.mountpoints[0] : null);
        if (!mount) {
          showBanner('Data partition not mounted. Cannot write config.', true, false);
          return;
        }
        try {
          showBanner('Writing config to target...', false, true);
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const defaultEntry = localStorage.getItem('raidhos_default_entry');
          const payload = {
            default_entry: defaultEntry || null,
            entries: renderedEntries.map((entry) => ({
              title: entry.title,
              path: entry.subtitle || entry.path,
              params: entry.params || '',
              initrd: entry.initrd || '',
              kargs: entry.kargs || '',
              class: entry.class || '',
              tip: entry.tip || '',
              hidden: !!entry.hidden,
              persistence_backend: entry.persistence_backend || '',
            })),
            tree_view: !!(globalTreeView && globalTreeView.checked),
            enable_disk_browser:
              !!(globalEnableDiskBrowser && globalEnableDiskBrowser.checked),
            grub_superuser:
              (globalGrubSuperuser && globalGrubSuperuser.value) || '',
            grub_password_pbkdf2:
              (globalGrubPasswordPbkdf2 && globalGrubPasswordPbkdf2.value) || '',
          };
          await invoke('write_boot_config_to_device', { mountPath: mount, config: payload });
          showBanner('Config written to target.', false, false);
        } catch (_err) {
          showBanner('Failed to write config to target.', true, false);
        }
      }

      async function writeGrubCfgToEsp() {
        if (!selectedEspMount) {
          showBanner('ESP not mounted. Cannot write grub.cfg.', true, false);
          return;
        }
        try {
          const invoke = tauriInvoke(); if (!invoke) throw new Error("Tauri runtime not available");
          const defaultEntry = localStorage.getItem('raidhos_default_entry');
          const payload = {
            default_entry: defaultEntry || null,
            entries: renderedEntries.map((entry) => ({
              title: entry.title,
              path: entry.path || mapEntryPath(entry),
              params: entry.params || '',
              initrd: entry.initrd || '',
              kargs: entry.kargs || '',
            })),
          };
          await invoke('write_grub_cfg_to_esp', { espMount: selectedEspMount, config: payload, dataLabel: 'DATA' });
        } catch (_err) {
          showBanner('Failed to write grub.cfg to ESP.', true, false);
        }
      }

      function persistState() {
        if (scanPathsInput) {
          localStorage.setItem('raidhos_scan_paths', scanPathsInput.value);
        }
        if (acceptWrite) {
          localStorage.setItem('raidhos_accept_write', String(acceptWrite.checked));
        }
        if (enableWrite) {
          localStorage.setItem('raidhos_enable_write', String(enableWrite.checked));
        }
        if (saveOnInstallOnly) {
          localStorage.setItem('raidhos_save_on_install_only', String(saveOnInstallOnly.checked));
        }
        if (globalTreeView) {
          localStorage.setItem('raidhos_tree_view', String(globalTreeView.checked));
        }
        if (globalEnableDiskBrowser) {
          localStorage.setItem('raidhos_enable_disk_browser', String(globalEnableDiskBrowser.checked));
        }
        if (globalGrubSuperuser) {
          localStorage.setItem('raidhos_grub_superuser', globalGrubSuperuser.value || '');
        }
        // grub_password_pbkdf2 is intentionally NOT written to
        // localStorage. The PBKDF2 hash enables offline
        // brute-force against the password, and localStorage is
        // reachable from any same-origin XSS even under our
        // strict CSP. The hash already flows through
        // save_boot_config → boot.json on disk, so the user's
        // configured password persists across sessions there.
      }

      if (scanPathsInput) scanPathsInput.addEventListener('change', persistState);
      if (acceptWrite) acceptWrite.addEventListener('change', persistState);
      if (enableWrite) enableWrite.addEventListener('change', persistState);
      if (saveOnInstallOnly) saveOnInstallOnly.addEventListener('change', persistState);
      if (globalTreeView) {
        globalTreeView.addEventListener('change', () => {
          persistState();
          pushBootConfig();
        });
      }
      if (globalEnableDiskBrowser) {
        globalEnableDiskBrowser.addEventListener('change', () => {
          persistState();
          pushBootConfig();
        });
      }
      if (globalGrubSuperuser) {
        globalGrubSuperuser.addEventListener('input', () => {
          persistState();
          pushBootConfig();
        });
      }
      if (globalGrubPasswordPbkdf2) {
        globalGrubPasswordPbkdf2.addEventListener('input', () => {
          persistState();
          pushBootConfig();
        });
      }

      // Sidebar nav drives the multi-view shell. Each `.view` block
      // owns its DOM; the sidebar item with the matching
      // `data-view` attribute toggles visibility. The About item
      // pops a modal rather than swapping the view.
      (function setupSidebar() {
        const items = Array.from(document.querySelectorAll('.sidebar-item'));
        const views = Array.from(document.querySelectorAll('.view'));
        const viewTitleEl = document.getElementById('viewTitle');
        const topSubtitleElLocal = document.getElementById('topSubtitle');
        const aboutModal = document.getElementById('aboutModal');
        const closeAbout = document.getElementById('closeAbout');
        const titleByView = {
          flash: ['Flash a USB', 'Build a multi-ISO bootable USB.'],
          settings: ['Settings', 'Boot config + GRUB security.'],
          logs: ['Logs', 'Install pipeline output.'],
        };

        function showView(name) {
          views.forEach((v) => {
            v.hidden = v.getAttribute('data-view') !== name;
          });
          if (viewTitleEl && titleByView[name]) {
            viewTitleEl.textContent = titleByView[name][0];
            if (topSubtitleElLocal) {
              topSubtitleElLocal.textContent = titleByView[name][1];
            }
          }
        }

        function showAbout() {
          if (!aboutModal) return;
          aboutModal.classList.add('open');
        }
        function hideAbout() {
          if (!aboutModal) return;
          aboutModal.classList.remove('open');
        }

        items.forEach((btn) => {
          btn.addEventListener('click', () => {
            const view = btn.getAttribute('data-view');
            if (view === 'about') {
              showAbout();
              return;
            }
            items.forEach((b) => {
              b.classList.toggle('active', b === btn);
              if (b === btn) {
                b.setAttribute('aria-current', 'page');
              } else {
                b.removeAttribute('aria-current');
              }
            });
            showView(view);
          });
        });

        if (closeAbout) closeAbout.addEventListener('click', hideAbout);
        if (aboutModal) {
          aboutModal.addEventListener('click', (e) => {
            if (e.target === aboutModal) hideAbout();
          });
        }
        document.addEventListener('keydown', (e) => {
          if (e.key === 'Escape') hideAbout();
        });
      })();

      restoreState();
      updateInstallPlan();
      loadPayloadVersion();
      // Fetch host metadata before loadCachedEntries so the empty-
      // state copy renders with the right OS paths on first paint.
      loadHostInfo();
      loadCachedEntries();
      updateInstallState();
      loadEntries();
      bootSequence();

      function updateLastSaved() {
        if (!lastSavedEl) return;
        const now = new Date();
        lastSavedEl.textContent = `Last saved: ${now.toLocaleString()}`;
        localStorage.setItem('raidhos_last_saved', now.toISOString());
      }

      function restoreLastSaved() {
        if (!lastSavedEl) return;
        const raw = localStorage.getItem('raidhos_last_saved');
        if (!raw) return;
        const date = new Date(raw);
        if (isNaN(date.getTime())) return;
        lastSavedEl.textContent = `Last saved: ${date.toLocaleString()}`;
      }

      restoreLastSaved();

      // ---------------------------------------------------------------
      // Drag-and-drop ISO support.
      //
      // Tauri 1 emits `tauri://file-drop` on the window; Tauri 2 emits
      // `tauri://drag-drop` with the same payload shape. We listen for
      // both so the frontend works on either backend.
      //
      // Step 1 UX: dropping an ISO adds it to the entry list directly
      // (no USB selection required). Step 2 UX (after a USB is
      // selected) also copies the file onto the mounted data
      // partition. The dispatch lives in ingestDroppedIsos so the
      // dialog "Browse for ISOs…" path reuses it.
      // ---------------------------------------------------------------
      async function ingestDroppedIsos(rawPaths) {
        const dropZone = document.getElementById('app');
        const paths = (Array.isArray(rawPaths) ? rawPaths : [])
          .filter((p) => typeof p === 'string' && p.toLowerCase().endsWith('.iso'));
        if (!paths.length) {
          showBanner('No .iso files in the drop — only .iso is accepted.', true, false);
          return;
        }
        // Merge into the in-memory entry list with verification
        // badges. The deduped existing entries keep their order.
        const existing = new Set(renderedEntries.map((e) => e.subtitle));
        const additions = paths
          .filter((p) => !existing.has(p))
          .map((p) => ({
            title: p.split(/[\\/]/).pop().replace(/\.iso$/i, ''),
            subtitle: p,
            tag: 'ISO',
            params: 'quiet splash',
            initrd: '',
            kargs: '',
          }));
        if (additions.length) {
          const merged = renderedEntries.concat(additions);
          hydrateEntryParams(merged);
          renderEntries(merged);
          showBanner(`Added ${additions.length} ISO(s) to entries.`, false, false);
        } else {
          showBanner('Those ISOs are already in the entry list.', false, false);
        }
        // If a USB target is already mounted, also copy onto it.
        if (selectedDataMount) {
          try {
            const inv = tauriInvoke();
            if (!inv) throw new Error('Tauri runtime not available');
            const copied = await inv(
              'copy_isos_to_data',
              { mountPath: selectedDataMount, sources: paths }
            );
            showBanner(`Copied ${copied.length} ISO(s) to USB.`, false, false);
          } catch (err) {
            showBanner(`Copy to USB failed: ${err}`, true, false);
          }
        }
        if (dropZone) dropZone.classList.remove('dragging');
      }

      (function setupDragAndDrop() {
        const dropZone = document.getElementById('app');
        if (!dropZone) return;

        // Browser-level events give us the visual hover state.
        // We always prevent the browser default so the webview
        // doesn't navigate away from the app if the Tauri event
        // chain misses for some reason.
        ['dragenter', 'dragover'].forEach((name) => {
          dropZone.addEventListener(name, (e) => {
            e.preventDefault();
            dropZone.classList.add('dragging');
          });
        });
        ['dragleave', 'dragend'].forEach((name) => {
          dropZone.addEventListener(name, () => {
            dropZone.classList.remove('dragging');
          });
        });
        dropZone.addEventListener('drop', (e) => {
          // Belt-and-braces: also try to read paths from the
          // browser DataTransfer in case the Tauri event misses.
          // Webview2/macOS Safari often give us full paths here.
          e.preventDefault();
          dropZone.classList.remove('dragging');
          const dt = e.dataTransfer;
          if (!dt) return;
          const fallback = [];
          for (let i = 0; i < dt.files.length; i++) {
            const f = dt.files[i];
            // `path` is a non-standard webkit field — present in
            // Tauri's WKWebView on macOS and Webview2 on Windows.
            if (f && typeof f.path === 'string') fallback.push(f.path);
          }
          if (fallback.length) ingestDroppedIsos(fallback);
        });

        // Tauri event delivery. Listen for both spellings (Tauri 1
        // and Tauri 2). `event.listen` is async-returning, but the
        // handler registers synchronously inside.
        const ev = (window.__TAURI__ && window.__TAURI__.event) || null;
        if (ev && typeof ev.listen === 'function') {
          const dropHandler = async (event) => {
            const payload = event && event.payload;
            // Tauri 2 payload: { type: 'drop'|'over'|'leave', paths, position }
            // Tauri 1 payload: ["...paths..."]
            let rawPaths = [];
            if (payload && Array.isArray(payload.paths)) {
              rawPaths = payload.paths;
            } else if (Array.isArray(payload)) {
              rawPaths = payload;
            }
            if (rawPaths.length) await ingestDroppedIsos(rawPaths);
          };
          ev.listen('tauri://file-drop', dropHandler).catch(() => {});
          ev.listen('tauri://drag-drop', dropHandler).catch(() => {});
          // Hover events from Tauri so the dragging class flips
          // even when the browser-level dragover doesn't fire
          // (it sometimes doesn't on Tauri 2 macOS).
          const overHandler = () => dropZone.classList.add('dragging');
          const leaveHandler = () => dropZone.classList.remove('dragging');
          ev.listen('tauri://drag-over', overHandler).catch(() => {});
          ev.listen('tauri://drag-enter', overHandler).catch(() => {});
          ev.listen('tauri://drag-leave', leaveHandler).catch(() => {});
          ev.listen('tauri://file-drop-hover', overHandler).catch(() => {});
          ev.listen('tauri://file-drop-cancelled', leaveHandler).catch(() => {});
        }
      })();

      // ---------------------------------------------------------------
      // Push progress events. Backend emits `raidhos://progress`;
      // replaces the previous Mutex<Vec> polling pattern.
      // ---------------------------------------------------------------
      (function setupProgressEvents() {
        if (!(window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen)) {
          return;
        }
        window.__TAURI__.event.listen('raidhos://progress', (event) => {
          const ev = event.payload || {};
          if (typeof progressEl === 'undefined' || !progressEl) return;
          const line = document.createElement('div');
          line.className = 'progress-line';
          line.setAttribute('role', 'status');
          const pct = ev.percent != null ? ` ${ev.percent}%` : '';
          line.textContent = `${ev.phase}: ${ev.message}${pct}`;
          progressEl.appendChild(line);
          // Auto-scroll the latest progress into view.
          progressEl.scrollTop = progressEl.scrollHeight;
        });
      })();

      // ---------------------------------------------------------------
      // Guided ("wizard") mode. Two-step flow:
      //   1. Pick ISO entries (the existing entries section).
      //   2. Pick USB and confirm (the install panel).
      //
      // Triggered by the toggle in the wizard-nav. Defaults to ON
      // for first-time users (the happy path is the headline UX);
      // power-users can flip it off and the choice persists in
      // localStorage.
      // ---------------------------------------------------------------
      (function setupWizard() {
        const toggle = document.getElementById('wizardToggle');
        const steps = Array.from(document.querySelectorAll('.wizard-step'));
        const panels = Array.from(document.querySelectorAll('[data-wizard-step]'));
        if (!toggle || steps.length === 0) return;

        const STATE_KEY = 'raidhos_wizard_mode';
        const STEP_KEY = 'raidhos_wizard_step';

        function applyWizardMode(on) {
          document.body.classList.toggle('wizard-mode', on);
          toggle.checked = on;
          if (on) {
            setStep(parseInt(localStorage.getItem(STEP_KEY) || '1', 10));
          } else {
            // Reveal all panels.
            panels.forEach((p) => p.classList.add('active'));
            steps.forEach((s) => s.classList.remove('active', 'done'));
          }
          // In expert (non-wizard) mode the Boot Config card is part
          // of the right-hand bento column; auto-open it so the user
          // can see what they opted into. In wizard mode it's hidden
          // anyway, so leave its state alone.
          const advanced = document.querySelector('.card.boot-config > details');
          if (advanced && !on) {
            advanced.open = true;
          }
        }

        function setStep(n) {
          n = Math.max(1, Math.min(2, n | 0));
          localStorage.setItem(STEP_KEY, String(n));
          panels.forEach((p) => {
            const s = parseInt(p.getAttribute('data-wizard-step') || '0', 10);
            p.classList.toggle('active', s === n);
          });
          steps.forEach((btn) => {
            const s = parseInt(btn.getAttribute('data-step') || '0', 10);
            btn.classList.toggle('active', s === n);
            btn.classList.toggle('done', s < n);
            btn.setAttribute('aria-selected', s === n ? 'true' : 'false');
          });
          // Light up the rail in brand once step 1 is no longer
          // the current step (i.e. the user has progressed). The
          // class lives on .wizard-nav so the rail CSS can react.
          const nav = document.querySelector('.wizard-nav');
          if (nav) nav.classList.toggle('step-1-done', n >= 2);
        }

        // Also light the rail proactively when step 1 has at least
        // one ISO — the user "completed" step 1 even if they
        // haven't clicked into step 2 yet. Hook off renderEntries.
        const origRender = window.renderEntries;
        document.addEventListener('raidhos:entries-updated', () => {
          const nav = document.querySelector('.wizard-nav');
          if (nav) nav.classList.toggle('step-1-done', renderedEntries.length > 0);
        });

        steps.forEach((btn) => {
          btn.addEventListener('click', () => {
            if (!document.body.classList.contains('wizard-mode')) return;
            const n = parseInt(btn.getAttribute('data-step') || '1', 10);
            setStep(n);
          });
        });

        toggle.addEventListener('change', () => {
          localStorage.setItem(STATE_KEY, toggle.checked ? '1' : '0');
          applyWizardMode(toggle.checked);
        });

        // First-time users default to Guided mode ON. Once a user
        // explicitly toggles, the explicit choice wins from then on.
        const saved = localStorage.getItem(STATE_KEY);
        const initial = saved === null ? true : saved === '1';
        applyWizardMode(initial);
      })();
