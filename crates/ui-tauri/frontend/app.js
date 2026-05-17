      // Tauri 1 -> 2 compatibility shim. Tauri 2 moved the invoke API
      // from `__TAURI__.tauri.invoke` to `__TAURI__.core.invoke` and
      // renamed file-drop events. Bridge both so the rest of the
      // frontend doesn't have to care which Tauri version it runs on.
      if (window.__TAURI__ && !window.__TAURI__.tauri && window.__TAURI__.core) {
        window.__TAURI__.tauri = { invoke: window.__TAURI__.core.invoke };
      }

      const entriesEl = document.getElementById('entries');
      const entriesConfigEl = document.getElementById('entriesConfig');
      const targetsEl = document.getElementById('targets');
      const saveConfigBtn = document.getElementById('saveConfig');
      const lastSavedEl = document.getElementById('lastSaved');
      const loadBtn = document.getElementById('load');
      const refreshBtn = document.getElementById('refresh');
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
      const payloadLabel = document.getElementById('payloadLabel');
      const configBanner = document.getElementById('configBanner');
      const resetParamsBtn = document.getElementById('resetParams');
      const saveOnInstallOnly = document.getElementById('saveOnInstallOnly');
      const resetModal = document.getElementById('resetModal');
      const cancelReset = document.getElementById('cancelReset');
      const confirmReset = document.getElementById('confirmReset');
      const globalTreeView = document.getElementById('globalTreeView');
      const globalGrubSuperuser = document.getElementById('globalGrubSuperuser');
      const globalGrubPasswordPbkdf2 = document.getElementById('globalGrubPasswordPbkdf2');

      let selectedDisk = null;
      let selectedEspMount = null;
      let selectedDataMount = null;
      let renderedTargets = [];
      let renderedEntries = [];
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
        if (!entries.length) {
          entriesEl.innerHTML = '<div class="disk">No entries detected.</div>';
          if (entryNote) entryNote.textContent = 'No ISO images found in default paths.';
          return;
        }
        entries.forEach((entry) => {
          const el = document.createElement('div');
          el.className = 'disk';
          el.innerHTML = `
            <div>
              <strong>${entry.title}</strong>
              <div><small>${entry.subtitle}</small></div>
            </div>
            <div class="pill">${entry.tag}</div>
          `;
          el.addEventListener('click', () => selectEntry(entry, el));
          entriesEl.appendChild(el);
        });

        if (entriesConfigEl) {
          renderConfigEntries(entries);
        }

        if (entryNote) entryNote.textContent = 'Entries sourced from ISO files.';
        startBootTimer();
        pushBootConfig();
        updateDefaultLabel();
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
          const tag = disk.is_system ? 'System' : (disk.removable ? 'Removable' : 'Fixed');
          el.innerHTML = `
            <div>
              <strong>${disk.id}</strong>
              <div><small>${disk.model || 'Unknown model'} · ${formatBytes(disk.size_bytes)}${mounts}</small></div>
            </div>
            <div class="pill">${tag}</div>
          `;
          if (disk.is_system) {
            el.style.opacity = '0.5';
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
        if (installBtn) {
          installBtn.textContent = enableWrite && enableWrite.checked && allowWriteOk
            ? 'Run Install'
            : 'Run Dry-Run Install';
        }
        if (enableWrite && enableWrite.checked && !allowWriteOk) {
          installBtn.disabled = true;
        }
        updateInstallPlan();
      }

      async function listDisks() {
        try {
          const { invoke } = window.__TAURI__.tauri;
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
          const { invoke } = window.__TAURI__.tauri;
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
          const { invoke } = window.__TAURI__.tauri;
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
      loadBtn.addEventListener('click', listDisks);
      refreshBtn.addEventListener('click', listDisks);
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
          const { invoke } = window.__TAURI__.tauri;
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

      async function loadPayloadVersion() {
        if (!payloadLabel) return;
        try {
          const { invoke } = window.__TAURI__.tauri;
          const version = await invoke('get_payload_version');
          payloadLabel.textContent = `Payload: ${version}`;
        } catch (_err) {
          payloadLabel.textContent = 'Payload: missing';
          showBanner('Payload manifest not found. Set payload/manifest.json.', true, false);
        }
      }

      function parseScanDirs() {
        const raw = (scanPathsInput && scanPathsInput.value) ? scanPathsInput.value : '';
        const defaults = ['/media', '/mnt', '/home'];
        if (!raw.trim()) return defaults;
        return raw
          .split(',')
          .map((s) => s.trim())
          .filter((s) => s.length > 0);
      }

      async function loadEntries() {
        try {
          const { invoke } = window.__TAURI__.tauri;
          const dirs = parseScanDirs();
          const isos = await invoke('scan_isos', { dirs });
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
        if (globalGrubSuperuser) {
          globalGrubSuperuser.value = localStorage.getItem('raidhos_grub_superuser') || '';
        }
        if (globalGrubPasswordPbkdf2) {
          globalGrubPasswordPbkdf2.value = localStorage.getItem('raidhos_grub_password_pbkdf2') || '';
        }
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
          const { invoke } = window.__TAURI__.tauri;
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
          const { invoke } = window.__TAURI__.tauri;
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
          const { invoke } = window.__TAURI__.tauri;
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
        if (globalGrubSuperuser) {
          localStorage.setItem('raidhos_grub_superuser', globalGrubSuperuser.value || '');
        }
        if (globalGrubPasswordPbkdf2) {
          localStorage.setItem('raidhos_grub_password_pbkdf2', globalGrubPasswordPbkdf2.value || '');
        }
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

      restoreState();
      updateInstallPlan();
      loadPayloadVersion();
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
      // Tauri 1 emits a `tauri://file-drop` event on the window when
      // a file is dropped into the webview from the host file
      // manager. HTML5 drag-and-drop alone doesn't give us paths in
      // a sandboxed webview, so we rely on Tauri's event.
      // ---------------------------------------------------------------
      (function setupDragAndDrop() {
        const dropZone = document.getElementById('app');
        if (!dropZone) return;
        const banner = document.createElement('div');
        banner.className = 'drop-banner';
        banner.setAttribute('role', 'status');
        banner.setAttribute('aria-live', 'polite');
        banner.style.display = 'none';
        banner.textContent = 'Drop ISOs to copy to the selected USB.';
        dropZone.appendChild(banner);

        // Visual hover state (browser-level events) — only the path
        // delivery uses Tauri's event.
        dropZone.addEventListener('dragover', (e) => {
          e.preventDefault();
          dropZone.classList.add('dragging');
          banner.style.display = 'block';
        });
        dropZone.addEventListener('dragleave', () => {
          dropZone.classList.remove('dragging');
          banner.style.display = 'none';
        });

        if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
          const dropHandler = async (event) => {
            dropZone.classList.remove('dragging');
            banner.style.display = 'none';
            // Tauri 2 payload shape: { paths: ["...", "..."] }
            // Tauri 1 payload shape: ["...", "..."]
            const rawPaths = (event.payload && event.payload.paths)
              ? event.payload.paths
              : (event.payload || []);
            const paths = (Array.isArray(rawPaths) ? rawPaths : []).filter((p) =>
              typeof p === 'string' && p.toLowerCase().endsWith('.iso')
            );
            if (!paths.length) return;
            if (!selectedDataMount) {
              banner.style.display = 'block';
              banner.textContent =
                'Select and mount a target USB first, then drop ISOs.';
              return;
            }
            try {
              banner.style.display = 'block';
              banner.textContent = `Copying ${paths.length} ISO(s)…`;
              const copied = await window.__TAURI__.tauri.invoke(
                'copy_isos_to_data',
                { mountPath: selectedDataMount, sources: paths }
              );
              banner.textContent = `Copied ${copied.length} ISO(s).`;
              setTimeout(() => {
                banner.style.display = 'none';
              }, 2500);
              if (typeof refreshIsos === 'function') refreshIsos();
            } catch (err) {
              banner.textContent = `Copy failed: ${err}`;
            }
          };
          // Register both the Tauri 1 and Tauri 2 event names so the
          // frontend works on either backend.
          window.__TAURI__.event.listen('tauri://file-drop', dropHandler);
          window.__TAURI__.event.listen('tauri://drag-drop', dropHandler);
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
      // Triggered by the toggle in the wizard-nav. Defaults to off so
      // power-users see the full dashboard. State persists in
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
        }

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

        applyWizardMode(localStorage.getItem(STATE_KEY) === '1');
      })();
