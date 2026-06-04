const tauri = window.__TAURI__ || {};
const { invoke } = tauri.core || {};
const { ask, message, open, save: saveDialog } = tauri.dialog || {};
const { readTextFile, writeTextFile } = tauri.fs || {};

let listen = null;

function setupListen() {
    if (tauri.event && typeof tauri.event.listen === 'function') {
        listen = tauri.event.listen;
    } else if (tauri.core && typeof tauri.core.listen === 'function') {
        listen = tauri.core.listen;
    }
}

console.log('Tauri APIs initialized:', {
    hasInvoke: !!invoke,
    hasDialog: !!ask,
    hasFs: !!readTextFile,
    hasEvent: !!tauri.event,
    hasCore: !!tauri.core
});

// State
let profileMetadata = [];
let currentProfileId = null;
let commonConfig = '';
let systemHosts = '';
let multiSelect = false;

// DOM Elements
const profileList = document.getElementById('profile-list');
const editor = document.getElementById('editor');
const currentNameDisplay = document.getElementById('current-profile-name');
const multiToggle = document.getElementById('multi-select-toggle');
const saveBtn = document.getElementById('save-btn');
const renameBtn = document.getElementById('rename-btn');
const addBtn = document.getElementById('add-profile-btn');
const importBtn = document.getElementById('import-btn');
const importSwitchHostsBtn = document.getElementById('import-switchhosts-btn');
const exportBtn = document.getElementById('export-btn');
const refreshBtn = document.getElementById('refresh-btn');
const systemEditBtn = document.getElementById('system-edit-btn');
const settingsBtn = document.getElementById('settings-btn');
const settingsModalOverlay = document.getElementById('settings-modal-overlay');
const settingsCloseBtn = document.getElementById('settings-close-btn');

// Status Bar
const remoteStatusBar = document.getElementById('remote-status-bar');
const lastUpdateTimeEl = document.getElementById('last-update-time');
const nextUpdateTimeEl = document.getElementById('next-update-time');

// Modal Logic
const modalOverlay = document.getElementById('modal-overlay');
const modalTitle = document.getElementById('modal-title');
const modalInput = document.getElementById('modal-input');
const modalConfirm = document.getElementById('modal-confirm');
const modalCancel = document.getElementById('modal-cancel');
// New Fields
const modalUrl = document.getElementById('modal-url');
const autoUpdateFields = document.getElementById('auto-update-fields');
const modalIntervalValue = document.getElementById('modal-interval-value');
const modalIntervalUnit = document.getElementById('modal-interval-unit');
const remoteFields = document.getElementById('remote-fields');
const typeRadios = document.getElementsByName('profile-type');
const updateModeRadios = document.getElementsByName('update-mode');

let modalCallback = null;


function showPrompt(title, initialData, callback) {
    modalTitle.innerText = title;
    
    // Handle simple string (legacy) or object
    const data = typeof initialData === 'object' ? initialData : { name: initialData || '' };
    
    modalInput.value = data.name || '';
    
    // Reset or Fill extended fields
    if (data.isRemote) {
        typeRadios[1].checked = true; // Remote
        remoteFields.classList.remove('hidden');
        modalUrl.value = data.url || '';
        
        if (data.updateInterval) {
             updateModeRadios[1].checked = true; // Auto
             autoUpdateFields.classList.remove('hidden');
             
             // Convert seconds to best unit
             let sec = data.updateInterval;
             let unit = 1;
             if (sec % 86400 === 0) unit = 86400;
             else if (sec % 3600 === 0) unit = 3600;
             else if (sec % 60 === 0) unit = 60;
             
             modalIntervalUnit.value = unit.toString();
             modalIntervalValue.value = (sec / unit).toString();
        } else {
             updateModeRadios[0].checked = true; // Manual
             autoUpdateFields.classList.add('hidden');
             modalIntervalValue.value = '1';
             modalIntervalUnit.value = '3600';
        }
    } else {
        // Local Default
        typeRadios[0].checked = true; // Local
        remoteFields.classList.add('hidden');
        modalUrl.value = '';
        updateModeRadios[0].checked = true;
        autoUpdateFields.classList.add('hidden');
        modalIntervalValue.value = '1';
        modalIntervalUnit.value = '3600';
    }

    modalOverlay.classList.remove('hidden');
    modalInput.focus();
    modalCallback = callback;
}


modalInput.onkeydown = (e) => {
    if (e.key === 'Enter') {
        modalConfirm.click();
    } else if (e.key === 'Escape') {
        modalCancel.click();
    }
};

modalConfirm.onclick = () => {
    const name = modalInput.value;
    const isRemote = typeRadios[1].checked;
    const url = modalUrl.value;
    
    let interval = 0;
    if (updateModeRadios[1].checked) { // Auto
        const val = parseInt(modalIntervalValue.value, 10) || 0;
        const unit = parseInt(modalIntervalUnit.value, 10) || 1;
        interval = val * unit;
    }
    
    console.log('Modal Confirm:', { name, isRemote, url, interval });
    
    if (modalCallback) {
        modalCallback({ name, isRemote, url, interval });
    }
    modalOverlay.classList.add('hidden');
};

// Type Toggle Logic
typeRadios.forEach(radio => {
    radio.onchange = () => {
        if (radio.value === 'remote') {
            remoteFields.classList.remove('hidden');
        } else {
            remoteFields.classList.add('hidden');
        }
    };
});

// Update Mode Toggle Logic
updateModeRadios.forEach(radio => {
    radio.onchange = () => {
        if (radio.value === 'auto') {
            autoUpdateFields.classList.remove('hidden');
        } else {
            autoUpdateFields.classList.add('hidden');
        }
    };
});


modalCancel.onclick = () => {
    modalOverlay.classList.add('hidden');
};

// Toast Logic
const toastContainer = document.getElementById('toast-container');

function showToast(text, type = 'info', duration = 3000) {
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    
    let icon = 'ℹ️';
    if (type === 'success') icon = '✅';
    if (type === 'error') icon = '❌';
    
    toast.innerHTML = `<span>${icon}</span><span>${text}</span>`;
    toastContainer.appendChild(toast);
    
    setTimeout(() => {
        toast.classList.add('fade-out');
        setTimeout(() => toast.remove(), 300);
    }, duration);
}

// Functions
async function loadData() {
    console.log('loadData starting...');
    try {
        if (!invoke) {
            console.error('Invoke not available!');
            return;
        }
        const config = await invoke('load_config');
        console.log('Config loaded:', config);
        
        profileMetadata = config.profiles || [];
        multiSelect = config.multi_select || false;
        multiToggle.checked = multiSelect;
        
        commonConfig = await invoke('load_common_config');
        console.log('Common config loaded');
        
        renderList();

        // 重建托盘菜单(包含最新 profile 列表)。失败也不影响主流程,静默。
        invoke('rebuild_tray_menu_cmd').catch(err => {
            console.warn('rebuild tray menu failed:', err);
        });

        // Refresh editor if common is active
        if (currentProfileId === 'common') {
            editor.value = commonConfig;
        } else if (currentProfileId && currentProfileId !== 'system') {
            const p = profileMetadata.find(x => x.id === currentProfileId);
            if (p) {
                const content = await invoke('list_profiles'); 
                const match = content.find(x => x.id === currentProfileId);
                if (match) editor.value = match.content;
            }
        }
    } catch (e) {
        console.error('loadData error:', e);
        showToast(`加载失败: ${e}`, 'error');
    }
}

function renderList() {
    profileList.innerHTML = '';
    profileMetadata.forEach(p => {
        const li = document.createElement('li');
        li.className = `profile-item ${p.id === currentProfileId ? 'active' : ''} ${p.active ? 'is-enabled' : ''}`;
        li.dataset.id = p.id;
        li.innerHTML = `
            <span class="status-dot"></span>
            <span class="name">
                ${p.url ? '☁️' : ''}${p.name}
            </span>
            <div class="row-actions">
                <span class="toggle-row-btn" title="${p.active ? '禁用' : '启用'}">${p.active ? '禁' : '启'}</span>
                ${p.url ? '<span class="update-row-btn" title="立即更新">刷</span>' : '<span class="blank-btn"></span>'}
                <span class="delete-row-btn" title="删除">删</span>
            </div>
        `;
        
        li.onclick = (e) => {
            if (e.target.classList.contains('delete-row-btn')) {
                deleteProfile(p.id, p.name);
            } else if (e.target.classList.contains('toggle-row-btn')) {
                e.stopPropagation();
                toggleProfile(p.id);
            } else if (e.target.classList.contains('update-row-btn')) {
                e.stopPropagation();
                updateRemoteProfile(p.id, p.name);
            } else {
                selectProfile(p.id);
            }
        };
        
        li.ondblclick = () => toggleProfile(p.id);
        
        profileList.appendChild(li);
    });
}

async function updateRemoteProfile(id, name) {
    const confirmed = await ask(`更新会覆盖现有配置 "${name}"，是否继续？`, {
        title: '更新确认',
        kind: 'info',
    });
    if (confirmed) {
        showToast(`正在更新 "${name}"...`, 'info');
        try {
            await invoke('trigger_profile_update', { id });
            await loadData();
            // If currently selected, refresh editor content
            if (currentProfileId === id) {
                selectProfile(id);
            }
            showToast('更新成功', 'success');
        } catch (e) {
            console.error(e);
            showToast(`更新失败: ${e}`, 'error');
        }
    }
}


let statusBarTimer = null;
let lastAutoRefreshTime = 0;

function updateStatusBar(p) {
    if (p && p.url) {
        remoteStatusBar.classList.remove('hidden');
        
        const updateText = () => {
             // Last Update: Only update if timestamp changed to define DOM
             // This prevents re-creating DOM every second, which would kill hover state.
             const currentLastTs = p.last_update || 'never';
             if (lastUpdateTimeEl.dataset.ts !== currentLastTs) {
                 lastUpdateTimeEl.dataset.ts = currentLastTs;
                 
                 const labelSpan = document.createElement('span');
                 labelSpan.className = 'refresh-action';
                 labelSpan.innerText = '上次刷新';
                 labelSpan.onmouseenter = () => labelSpan.innerText = '马上刷新';
                 labelSpan.onmouseleave = () => labelSpan.innerText = '上次刷新';
                 labelSpan.onclick = () => manualRefreshRemote(p.id);
                 
                 let timeText = '从未';
                 if (p.last_update) {
                     timeText = formatDate(new Date(p.last_update));
                 }
                 
                 lastUpdateTimeEl.innerHTML = '';
                 lastUpdateTimeEl.appendChild(labelSpan);
                 lastUpdateTimeEl.appendChild(document.createTextNode(`：${timeText}`));
             }
            
            // Next Update
            let nextText = '';
            if (p.update_interval && p.update_interval > 0) {
                 let lastTime = p.last_update ? new Date(p.last_update) : null;
                 if (lastTime) {
                    const nextTime = new Date(lastTime.getTime() + p.update_interval * 1000);
                    // Check if overdue?
                    const now = new Date();
                    const diff =  nextTime - now;
                    
                    if (diff <= 1000) { // If <= 1s remaining
                         nextText = '正在更新...';
                         // Trigger check
                         const nowTs = Date.now();
                         if (nowTs - lastAutoRefreshTime > 2000) {
                             lastAutoRefreshTime = nowTs;
                             // Call loadData silently (no spinner on refresh button, but effective)
                             loadData();
                         }
                    } else {
                         nextText = `下次刷新：${formatDate(nextTime)} (还有 ${Math.floor(diff/1000)}秒)`;
                    }
                } else {
                    nextText = '下次刷新：即将进行';
                }
            } else {
                nextText = '自动刷新：未开启';
            }
            nextUpdateTimeEl.innerText = nextText;
        };
        
        updateText();
    } else {
        remoteStatusBar.classList.add('hidden');
    }
}

async function manualRefreshRemote(id) {
    if (!id) return;
    showToast('正在刷新...', 'info');
    try {
        await invoke('trigger_profile_update', { id });
        await loadData();
        // Force status bar update immediately with new data
        const p = profileMetadata.find(x => x.id === id);
        if (p) updateStatusBar(p);
        
        if (currentProfileId === id) {
             // Refresh editor content
             selectProfile(id);
        }
        showToast('刷新成功', 'success');
    } catch (e) {
        showToast(`刷新失败: ${e}`, 'error');
    }
}

function startStatusBarTimer(id) {
    if (statusBarTimer) clearInterval(statusBarTimer);
    statusBarTimer = null;
    
    if (!id || id === 'system' || id === 'common') {
        remoteStatusBar.classList.add('hidden');
        return;
    }
    
    // Check if remote
    const p = profileMetadata.find(x => x.id === id);
    if (p && p.url) {
        // Update immediately
        updateStatusBar(p);
        // Start timer
        statusBarTimer = setInterval(() => {
             // vital: re-find profile to get latest last_update if it changed
             const currentP = profileMetadata.find(x => x.id === id);
             if (currentP) updateStatusBar(currentP);
        }, 1000);
    } else {
        remoteStatusBar.classList.add('hidden');
    }
}

function formatDate(date) {
    const pad = (n) => n.toString().padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth()+1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

async function selectProfile(id) {
    currentProfileId = id;
    renameBtn.classList.add('hidden');
    systemEditBtn.classList.add('hidden');
    systemEditBtn.innerText = '编辑';
    
    // Reset Title Events
    currentNameDisplay.onmouseenter = null;
    currentNameDisplay.onmouseleave = null;
    currentNameDisplay.onclick = null;
    currentNameDisplay.classList.remove('exportable-title');
    currentNameDisplay.title = '';
    currentNameDisplay.style.color = '';

    const setupExportTitle = (name) => {
        currentNameDisplay.classList.add('exportable-title');
        currentNameDisplay.title = '点击导出此配置';
        currentNameDisplay.onmouseenter = () => {
             currentNameDisplay.innerText = `导出 ${name}`;
        }
        currentNameDisplay.onmouseleave = () => {
             currentNameDisplay.innerText = name;
        }
        currentNameDisplay.onclick = exportCurrentProfile;
    };

    if (id === 'system') {
        const displayName = '系统 Hosts (只读)';
        currentNameDisplay.innerText = displayName;
        editor.readOnly = true;
        saveBtn.classList.add('hidden');
        systemEditBtn.classList.remove('hidden');
        setupExportTitle(displayName);
        try {
            systemHosts = await invoke('get_system_hosts');
            editor.value = systemHosts;
        } catch (e) { console.error(e); }
    } else if (id === 'common') {
        const displayName = '公共配置 (Common)';
        currentNameDisplay.innerText = displayName;
        editor.readOnly = false;
        saveBtn.classList.remove('hidden');
        editor.value = commonConfig;
        setupExportTitle(displayName);
    } else {
        const p = profileMetadata.find(x => x.id === id);
        if (p) {
            currentNameDisplay.innerText = p.name;
            editor.readOnly = false;
            saveBtn.classList.remove('hidden');
            renameBtn.classList.remove('hidden');
            
            setupExportTitle(p.name);

            try {
                // We use list_profiles to get content since get_profile_content doesn't exist
                const all = await invoke('list_profiles');
                const match = all.find(x => x.id === id);
                if (match) editor.value = match.content;
            } catch (e) { console.error(e); }
            startStatusBarTimer(id);
        }
    }
    if (id === 'system' || id === 'common') {
        startStatusBarTimer(null); // Stop timer and hide bar
    }
    // Update active class for fixed list
    document.querySelectorAll('#fixed-list .profile-item').forEach(li => {
        if (li.dataset.id === id) {
            li.classList.add('active');
        } else {
            li.classList.remove('active');
        }
    });

    renderList(); // Update active class for custom profiles
}

async function saveCurrent() {
    if (!currentProfileId) return;
    const content = editor.value;
    
    try {
        if (currentProfileId === 'common') {
            await invoke('save_common_config', { content });
            commonConfig = content;
        } else if (currentProfileId === 'system') {
            await invoke('save_system_hosts', { content });
            systemEditBtn.innerText = '编辑';
            editor.readOnly = true;
            saveBtn.classList.add('hidden');
            systemEditBtn.classList.remove('hidden');
            showToast('已更新系统文件', 'success');
            return;
        } else {
            await invoke('save_profile_content', { id: currentProfileId, content });
        }
        showToast('保存成功', 'success');
    } catch (e) {
        showToast(`保存失败: ${e}`, 'error');
    }
}

async function toggleSystemEdit() {
    if (editor.readOnly) {
        editor.readOnly = false;
        editor.focus();
        systemEditBtn.classList.add('hidden');
        saveBtn.classList.remove('hidden');
        showToast('进入编辑模式', 'info');
    }
}

async function toggleProfile(id) {
    if (id === 'system' || id === 'common') return;
    try {
        await invoke('toggle_profile_active', { id });
        await loadData();
        
        // Find profile to show specific name in toast
        const config = await invoke('load_config');
        const p = config.profiles.find(x => x.id === id);
        if (p) {
            showToast(`${p.name} 已${p.active ? '启用' : '禁用'}`, 'success');
        }

        // If current view is system hosts, refresh immediately
        if (currentProfileId === 'system') {
            const systemContent = await invoke('get_system_hosts');
            editor.value = systemContent;
        }
    } catch (e) {
        showToast(`切换失败: ${e}`, 'error');
    }
}

async function createProfile(data) {
    // data is now an object: { name, isRemote, url, interval } or just string if legacy (but we updated showPrompt)
    let name = data;
    let extra = {};
    if (typeof data === 'object') {
        name = data.name;
        extra = data;
    }
    
    console.log('Creating profile:', name, extra);
    if (!name) return;
    try {
        let args = { name };
        if (extra.isRemote && extra.url) {
            args.url = extra.url;
            // args.updateInterval = extra.interval; // Tauri expects snake_case for rust args usually? 
            // Tauri 2.0 with rename_all="camelCase" is default? No, default is camelCase for JS -> snake_case for Rust variables?
            // Actually Tauri maps JS object keys to Rust arg names. Rust args are snake_case.
            // Tauri by default converts camelCase to snake_case.
            args.updateInterval = extra.interval; 
        }

        const id = await invoke('create_profile', args); 
        
        if (extra.isRemote && extra.url) {
             showToast('正在下载远程配置...', 'info');
             try {
                 await invoke('trigger_profile_update', { id });
                 showToast('远程配置下载成功', 'success');
             } catch (e) {
                 console.error('Download failed:', e);
                 showToast(`下载失败: ${e}`, 'error');
             }
        }

        console.log('Profile created, ID:', id);
        await loadData();
        selectProfile(id);
        showToast('创建成功 (部分加载中)', 'success');
    } catch (e) {
        console.error('Create profile error:', e);
        showToast(`创建失败: ${e}`, 'error');
    }
}

async function deleteProfile(id, name) {
    const confirmed = await ask(`确定要删除配置 "${name}" 吗？`, {
        title: '删除确认',
        kind: 'warning',
    });
    if (confirmed) {
        try {
            await invoke('delete_profile', { id });
            if (currentProfileId === id) {
                currentProfileId = null;
                editor.value = '';
                currentNameDisplay.innerText = '请选择配置';
            }
            await loadData();
            showToast('已删除', 'info');
        } catch (e) {
            showToast(`删除失败: ${e}`, 'error');
        }
    }
}

async function editProfile() {
    if (!currentProfileId || currentProfileId === 'system' || currentProfileId === 'common') return;
    const p = profileMetadata.find(x => x.id === currentProfileId);
    if (!p) return;
    
    // Preparation for showPrompt
    const initialData = {
        name: p.name,
        isRemote: !!p.url, // If has URL, assume remote type logic
        url: p.url,
        updateInterval: p.update_interval
    };
    
    showPrompt('修改配置', initialData, async (newData) => {
        // newData: { name, isRemote, url, interval }
        try {
            // 1. Rename if changed
            if (newData.name && newData.name !== p.name) {
                 await invoke('rename_profile', { id: p.id, newName: newData.name });
            }
            
            // 2. Update Remote Config
            // Determine new URL and Interval
            let newUrl = null;
            let newInterval = null;
            
            if (newData.isRemote) {
                newUrl = newData.url;
                // If interval > 0, set it. Otherwise None.
                if (newData.interval > 0) newInterval = newData.interval;
            }
            
            // Call backend to update metadata
            // Note: If switching Local -> Remote, or Remote -> Local (url=null), this handles it.
            await invoke('update_remote_config', { 
                id: p.id, 
                url: newUrl, 
                updateInterval: newInterval 
            });
            
            await loadData();
            currentNameDisplay.innerText = newData.name;
            showToast('配置已更新', 'success');
            
            // If it became remote and has URL, ask to update content? 
            // Or just let user click update button?
            // User might expect "Save" to apply new URL content immediately? 
            // Let's being conservative: if URL changed, trigger update.
            if (newData.isRemote && newData.url && newData.url !== p.url) {
                // Trigger download
                 showToast('正在下载新地址内容...', 'info');
                 await invoke('trigger_profile_update', { id: p.id });
                 showToast('内容已更新', 'success');
                 if (currentProfileId === p.id) selectProfile(p.id); // refresh editor with new content
            }
            
        } catch (e) {
            console.error(e);
            showToast(`修改失败: ${e}`, 'error');
        }
    });
}

async function toggleMultiSelect() {
    try {
        await invoke('set_multi_select', { enable: multiToggle.checked });
        multiSelect = multiToggle.checked;
        await loadData();
        showToast(multiSelect ? '多选模式已开启' : '多选模式已关闭');
    } catch (e) {
        console.error(e);
    }
}

async function importData() {
    const selected = await open({
        multiple: false,
        filters: [{ name: 'Data', extensions: ['json', 'txt', 'hosts'] }]
    });
    if (selected) {
        try {
            const content = await invoke('import_file', { path: selected });
            if (selected.endsWith('.json')) {
                await invoke('import_data', { jsonContent: content });
            } else {
                const name = selected.split(/[\/\\]/).pop().split('.')[0];
                await invoke('create_profile', { name, content });
            }
            await loadData();
            showToast('导入成功', 'success');
        } catch (e) {
            showToast(`导入失败: ${e}`, 'error');
        }
    }
}

async function importSwitchHosts() {
    try {
        const selected = await open({
            filters: [{ name: 'JSON', extensions: ['json'] }]
        });
        if (selected) {
            const data = await invoke('import_file', { path: selected });
            const count = await invoke('import_switchhosts', { jsonContent: data });
            await loadData();
            showToast(`已从 SwitchHosts 导入 ${count} 个环境`, 'success');
        }
    } catch (e) {
        showToast(`导入失败: ${e}`, 'error');
    }
}

async function exportAll() {
    const path = await saveDialog({
        defaultPath: 'hosts-backup.json',
        filters: [{ name: 'JSON', extensions: ['json'] }]
    });
    if (path) {
        try {
            const data = await invoke('export_data');
            // Use backend command to bypass frontend FS permissions
            await invoke('export_file', { path, content: data });
            showToast('导出成功', 'success');
        } catch (e) {
            showToast(`导出失败: ${e}`, 'error');
        }
    }
}

async function exportCurrentProfile() {
    let filename = 'hosts.txt';
    if (currentProfileId === 'system') filename = 'system-hosts.txt';
    else if (currentProfileId === 'common') filename = 'common-config.txt';
    else {
        const p = profileMetadata.find(x => x.id === currentProfileId);
        if (p) filename = `${p.name}.txt`;
        else return;
    }

    const path = await saveDialog({
        defaultPath: filename,
        filters: [{ name: 'Text', extensions: ['txt', 'hosts'] }]
    });
    
    if (path) {
        try {
             // Use current editor value (what you see is what you export)
             const content = editor.value;
             await invoke('export_file', { path, content });
             showToast('导出成功', 'success');
        } catch (e) {
             showToast(`导出失败: ${e}`, 'error');
        }
    }
}

// Fixed list clicks
document.querySelectorAll('#fixed-list .profile-item').forEach(li => {
    li.onclick = () => selectProfile(li.dataset.id);
});

async function refreshData() {
    refreshBtn.classList.add('spinning');
    await loadData();
    setTimeout(() => {
        refreshBtn.classList.remove('spinning');
        showToast('数据已刷新', 'info');
    }, 500);
}

const githubLink = document.getElementById('github-link');

// Event Listeners
saveBtn.onclick = saveCurrent;
renameBtn.onclick = editProfile; // renamed function
systemEditBtn.onclick = toggleSystemEdit;
addBtn.onclick = () => showPrompt('新建配置', '', createProfile);
multiToggle.onchange = toggleMultiSelect;
refreshBtn.onclick = (e) => {
    e.stopPropagation();
    refreshData();
};
importBtn.onclick = importData;
importSwitchHostsBtn.onclick = importSwitchHosts;
exportBtn.onclick = exportAll;

githubLink.onclick = () => {
    invoke('hostly_open_url', { url: 'https://github.com/cn07115/Hostlyy' });
};

// Theme Logic
// Theme Logic

// Tracks the user's selected mode (one of: 'light', 'dark', 'system').
// The actual applied theme is `applyThemeMode()` output, which expands
// 'system' to the current OS theme.
let selectedThemeMode = 'dark';

/**
 * Map a Tauri/webview theme value ('light' | 'dark') to our internal
 * data-theme attribute. Falls back to OS preference probe via matchMedia,
 * then to 'dark'.
 */
function resolveOsTheme(t) {
    if (t === 'light' || t === 'dark') return t;
    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) {
        return 'light';
    }
    return 'dark';
}

/** Read the current OS theme via Tauri (with matchMedia fallback). */
async function queryOsTheme() {
    try {
        const win = (tauri.window && tauri.window.getCurrentWindow)
            ? tauri.window.getCurrentWindow()
            : (tauri.webviewWindow && tauri.webviewWindow.getCurrentWebviewWindow
                ? tauri.webviewWindow.getCurrentWebviewWindow() : null);
        if (win && typeof win.theme === 'function') {
            const t = await win.theme();
            return resolveOsTheme(t);
        }
    } catch (e) {
        // fall through to matchMedia
    }
    return resolveOsTheme(null);
}

/**
 * Apply `selectedThemeMode` to the DOM. If mode is 'system', ask Tauri
 * (or matchMedia) for the current OS theme and use that. This is the
 * single function that touches data-theme, so any path that wants to
 * re-apply the current mode (init, OS-theme-change, manual radio click)
 * just calls it.
 */
async function applyThemeMode() {
    let effective = selectedThemeMode;
    if (effective === 'system') {
        effective = await queryOsTheme();
    }
    document.documentElement.dataset.theme = effective;
    localStorage.setItem('hostly-theme', selectedThemeMode); // store the *mode*, not the resolved value
    // Sync the radio selection
    const radios = document.getElementsByName('theme-mode');
    radios.forEach(r => { if (r.value === selectedThemeMode) r.checked = true; });
}

async function initTheme() {
    try {
        const config = await invoke('load_config');
        selectedThemeMode = config.theme || localStorage.getItem('hostly-theme') || 'dark';
        // Validate stored value
        if (!['light', 'dark', 'system'].includes(selectedThemeMode)) selectedThemeMode = 'dark';
        await applyThemeMode();
        // Persist if we fell back to a default (so the backend config matches DOM)
        if (!config.theme) {
            try { await invoke('set_theme', { theme: selectedThemeMode }); } catch (_) {}
        }
    } catch (e) {
        console.error('Failed to load theme config:', e);
        const saved = localStorage.getItem('hostly-theme') || 'dark';
        selectedThemeMode = ['light', 'dark', 'system'].includes(saved) ? saved : 'dark';
        await applyThemeMode();
    }
    // Subscribe to OS theme changes so 'system' mode stays in sync.
    // 双轨:matchMedia 是 webview 通用兜底(WebView2/WebKit/WebKitGTK 都支持);
    // Tauri 的 onThemeChanged 在某些 webview 上不靠谱,作为补充。
    try {
        if (window.matchMedia) {
            const mq = window.matchMedia('(prefers-color-scheme: light)');
            const onChange = () => {
                if (selectedThemeMode === 'system') applyThemeMode();
            };
            if (typeof mq.addEventListener === 'function') {
                mq.addEventListener('change', onChange);
            } else if (typeof mq.addListener === 'function') {
                // Old Safari
                mq.addListener(onChange);
            }
        }
    } catch (e) {
        // OS theme tracking is best-effort
    }
    try {
        const win = (tauri.window && tauri.window.getCurrentWindow)
            ? tauri.window.getCurrentWindow()
            : (tauri.webviewWindow && tauri.webviewWindow.getCurrentWebviewWindow
                ? tauri.webviewWindow.getCurrentWebviewWindow() : null);
        if (win && typeof win.onThemeChanged === 'function') {
            await win.onThemeChanged(() => {
                if (selectedThemeMode === 'system') {
                    applyThemeMode();
                }
            });
        }
    } catch (e) {
        // OS theme tracking is best-effort
    }
}

async function setTheme(mode, persist = true) {
    selectedThemeMode = mode;
    await applyThemeMode();
    if (persist) {
        try {
            await invoke('set_theme', { theme: mode });
        } catch (e) {
            console.error('Failed to save theme:', e);
        }
    }
}

// Settings Modal Logic
settingsBtn.onclick = () => {
    settingsModalOverlay.classList.remove('hidden');

    Promise.all([
        initWindowSettings(),
        initSystemSettings(),
        initAboutPane()
    ]);
};

// Settings tab switching
document.querySelectorAll('.settings-tab').forEach(tab => {
    tab.onclick = () => {
        const target = tab.dataset.tab;
        document.querySelectorAll('.settings-tab').forEach(t => {
            t.classList.toggle('active', t === tab);
        });
        document.querySelectorAll('.settings-pane').forEach(p => {
            p.classList.toggle('active', p.id === `tab-${target}`);
        });
    };
});

// About pane
async function initAboutPane() {
    try {
        const version = await invoke('get_app_version');
        const vEl = document.getElementById('about-version');
        if (vEl) vEl.textContent = version;
    } catch (e) {
        console.error('Failed to load version:', e);
    }
    // 同步 localStorage 的代理设置到复选框/文本框
    initUpdateProxyUI();
}

const aboutGithubLink = document.getElementById('about-github-link');
if (aboutGithubLink) {
    aboutGithubLink.onclick = (e) => {
        e.preventDefault();
        invoke('hostly_open_url', { url: 'https://github.com/cn07115/Hostlyy' });
    };
}

const aboutCopyGithub = document.getElementById('about-copy-github');
if (aboutCopyGithub) {
    aboutCopyGithub.onclick = async () => {
        try {
            await navigator.clipboard.writeText('https://github.com/cn07115/Hostlyy');
            showToast('已复制 GitHub 地址', 'success');
        } catch (e) {
            showToast(`复制失败: ${e}`, 'error');
        }
    };
}

// =============== 代理更新设置 (localStorage 持久化) ===============
// 启动检查 + 手动检查共用同一套配置;首次安装未改过设置时默认走直连(海外/能上 GitHub 不打扰)
const PROXY_USE_KEY = 'hostly-update-use-proxy';
const PROXY_BASE_KEY = 'hostly-update-proxy-base';
const PROXY_BASE_DEFAULT = 'https://gh.xmly.dev/';

function getProxySettings() {
    return {
        useProxy: localStorage.getItem(PROXY_USE_KEY) === '1',
        proxyBase: localStorage.getItem(PROXY_BASE_KEY) || PROXY_BASE_DEFAULT
    };
}

function saveProxySettings(useProxy, proxyBase) {
    localStorage.setItem(PROXY_USE_KEY, useProxy ? '1' : '0');
    localStorage.setItem(PROXY_BASE_KEY, proxyBase);
}

const aboutCheckUpdateBtn = document.getElementById('about-check-update');
const aboutUpdateStatus = document.getElementById('about-update-status');
const aboutUpdateDetail = document.getElementById('about-update-detail');
const aboutUseProxyCheckbox = document.getElementById('about-use-proxy-checkbox');
const aboutProxyBaseInput = document.getElementById('about-proxy-base-input');

// 打开设置面板时把 localStorage 同步到 UI
function initUpdateProxyUI() {
    if (!aboutUseProxyCheckbox || !aboutProxyBaseInput) return;
    // 一次性迁移:老默认(ghproxy.com / ghfast.top)-> 新默认(gh.xmly.dev)
    // 精确匹配,不误伤用户手动填的值
    const stored = localStorage.getItem(PROXY_BASE_KEY);
    const oldDefaults = ['https://ghproxy.com/', 'https://ghfast.top/'];
    if (stored && oldDefaults.includes(stored)) {
        localStorage.setItem(PROXY_BASE_KEY, PROXY_BASE_DEFAULT);
    }
    const { useProxy, proxyBase } = getProxySettings();
    aboutUseProxyCheckbox.checked = useProxy;
    aboutProxyBaseInput.value = proxyBase;
}

if (aboutUseProxyCheckbox) {
    aboutUseProxyCheckbox.onchange = () => {
        saveProxySettings(aboutUseProxyCheckbox.checked, (aboutProxyBaseInput.value || '').trim() || PROXY_BASE_DEFAULT);
    };
}
if (aboutProxyBaseInput) {
    aboutProxyBaseInput.onchange = () => {
        saveProxySettings(!!(aboutUseProxyCheckbox && aboutUseProxyCheckbox.checked), (aboutProxyBaseInput.value || '').trim() || PROXY_BASE_DEFAULT);
    };
}

// 统一检查更新函数(供手动按钮 + 启动检查共用)
// 返回 { found:false } 或 { found:true, mode:'proxy'|'direct', version, notes, ... }
// - mode='proxy' 时额外有 downloadUrl
// - mode='direct' 时额外有 update(原 updater 对象)
async function doCheckUpdate({ useProxy, proxyBase }) {
    if (useProxy) {
        if (!proxyBase) throw new Error('请填写代理地址');
        const info = await invoke('check_update_with_proxy', { proxyBase });
        if (!info || !info.url) {
            return { found: false };
        }
        return {
            found: true,
            mode: 'proxy',
            version: info.version,
            notes: info.notes || '',
            downloadUrl: info.url,
        };
    }
    const updater = (tauri.updater && tauri.updater.check)
        ? tauri.updater
        : (window.__TAURI__ && window.__TAURI__.updater);
    if (!updater || typeof updater.check !== 'function') {
        throw new Error('更新插件未初始化');
    }
    const update = await updater.check();
    if (update === null) {
        return { found: false };
    }
    return {
        found: true,
        mode: 'direct',
        version: update.version,
        notes: update.notes || '',
        update,
    };
}

if (aboutCheckUpdateBtn) {
    aboutCheckUpdateBtn.onclick = async () => {
        aboutCheckUpdateBtn.disabled = true;
        aboutUpdateStatus.textContent = '检查中...';
        aboutUpdateStatus.style.color = 'var(--text-dim)';
        aboutUpdateDetail.textContent = '';
        try {
            // 同步 UI 状态到 localStorage(用户改了就立刻记住,启动检查直接复用)
            const useProxy = !!(aboutUseProxyCheckbox && aboutUseProxyCheckbox.checked);
            const proxyBase = (aboutProxyBaseInput && aboutProxyBaseInput.value || '').trim();
            saveProxySettings(useProxy, proxyBase || PROXY_BASE_DEFAULT);

            const result = await doCheckUpdate({ useProxy, proxyBase });
            if (!result.found) {
                aboutUpdateStatus.textContent = '已是最新版本';
                aboutUpdateStatus.style.color = '#3fb950';
            } else {
                aboutUpdateStatus.textContent = `发现新版本 ${result.version}`;
                aboutUpdateStatus.style.color = '#d29922';
                if (result.mode === 'proxy') {
                    aboutUpdateDetail.textContent = `即将打开下载链接(代理:${proxyBase})`;
                    await invoke('hostly_open_url', { url: result.downloadUrl });
                } else {
                    const yesNo = await ask(`发现新版本 ${result.version}，是否立即下载并安装？`, {
                        title: '更新可用',
                        kind: 'info',
                    });
                    if (yesNo) {
                        aboutUpdateStatus.textContent = '下载中...';
                        await result.update.downloadAndInstall();
                        aboutUpdateStatus.textContent = '已下载,重启后生效';
                        aboutUpdateStatus.style.color = '#3fb950';
                    }
                }
            }
        } catch (e) {
            console.error('Update check failed:', e);
            aboutUpdateStatus.textContent = '检查失败';
            aboutUpdateStatus.style.color = '#f85149';
            aboutUpdateDetail.textContent = String(e);
        } finally {
            aboutCheckUpdateBtn.disabled = false;
        }
    };
}

const closeSettings = () => settingsModalOverlay.classList.add('hidden');
settingsCloseBtn.onclick = closeSettings;
settingsModalOverlay.onclick = (e) => {
    if (e.target === settingsModalOverlay) closeSettings();
};

document.getElementsByName('theme-mode').forEach(radio => {
    radio.onchange = (e) => setTheme(e.target.value);
});


// Window Settings Logic
let currentWindowMode = 'remember';
let resizeTimeout;

async function initWindowSettings() {
    try {
        const config = await invoke('load_config');
        const mode = config.window_mode || 'remember';
        const w = config.window_width || 1000;
        const h = config.window_height || 700;
        
        applyWindowSettings(mode, w, h);
    } catch(e) { console.error(e); }
}

function applyWindowSettings(mode, w, h) {
    currentWindowMode = mode;
    const select = document.getElementById('window-size-select');
    const radios = document.getElementsByName('window-mode');
    
    radios.forEach(r => {
        if (r.value === mode) r.checked = true;
    });

    if (mode === 'fixed') {
        select.classList.remove('hidden');
        select.value = `${w},${h}`;
        // If not found, use custom
        if (!select.value && w && h) {
             const custom = select.querySelector('option[hidden]');
             if (custom) {
                 custom.value = `${w},${h}`;
                 custom.innerText = `${w} x ${h}`;
                 custom.selected = true;
             }
        }
    } else {
        select.classList.add('hidden');
        startResizeListener();
    }
}

function startResizeListener() {
    window.onresize = () => {
        if (currentWindowMode !== 'remember') return;
        clearTimeout(resizeTimeout);
        resizeTimeout = setTimeout(() => {
            saveWindowConfig('remember', window.outerWidth, window.outerHeight);
        }, 1000);
    };
}

async function saveWindowConfig(mode, w, h) {
    try {
        // Ensure w, h are numbers
        await invoke('save_window_config', { mode, width: parseFloat(w), height: parseFloat(h) });
    } catch(e) { console.error(e); }
}

// Listeners
document.getElementsByName('window-mode').forEach(r => {
    r.onchange = (e) => {
        const mode = e.target.value;
        currentWindowMode = mode;
        const select = document.getElementById('window-size-select');
        
        if (mode === 'fixed') {
            select.classList.remove('hidden');
            if (!select.value) select.selectedIndex = 0;
            const [w, h] = select.value.split(',');
            saveWindowConfig(mode, w, h);
        } else {
            select.classList.add('hidden');
            startResizeListener();
            saveWindowConfig(mode, window.outerWidth, window.outerHeight);
        }
    }
});

document.getElementById('window-size-select').onchange = (e) => {
    const val = e.target.value;
    const [w, h] = val.split(',');
    saveWindowConfig('fixed', w, h);
};

async function initSystemSettings() {
    try {
        const [autoStart, closeBehavior, rememberClose, syncStatus] = await Promise.all([
            invoke('get_auto_start'),
            invoke('get_close_behavior'),
            invoke('get_remember_close_choice'),
            invoke('get_sync_status').catch(() => null)
        ]);

        document.getElementById('auto-start-checkbox').checked = autoStart;

        const closeRadios = document.getElementsByName('close-behavior');
        closeRadios.forEach(r => {
            r.checked = r.value === closeBehavior;
        });

        document.getElementById('remember-close-checkbox').checked = rememberClose;
        updateRememberGroupVisibility(closeBehavior);

        if (syncStatus) {
            if (webdavUrlEl) webdavUrlEl.value = syncStatus.url || '';
            if (webdavUsernameEl) webdavUsernameEl.value = syncStatus.username || '';
            // Password is intentionally left empty (we don't read from keychain)
            if (webdavLastSyncEl) {
                webdavLastSyncEl.textContent = formatSyncTime(syncStatus.last_sync);
                const st = formatSyncStatus(syncStatus);
                if (webdavLastStatusEl) {
                    webdavLastStatusEl.textContent = st.text;
                    webdavLastStatusEl.style.color = st.color;
                }
            }
        }
    } catch(e) { console.error('Failed to load system settings:', e); }
}

function updateRememberGroupVisibility(closeBehavior) {
    const group = document.getElementById('remember-close-group');
    if (group) {
        group.classList.toggle('hidden', closeBehavior === 'exit');
    }
}

// =============== WebDAV Sync UI ===============

const webdavUrlEl = document.getElementById('webdav-url');
const webdavUsernameEl = document.getElementById('webdav-username');
const webdavPasswordEl = document.getElementById('webdav-password');
const webdavSaveBtn = document.getElementById('webdav-save-btn');
const webdavTestBtn = document.getElementById('webdav-test-btn');
const webdavSyncBtn = document.getElementById('webdav-sync-btn');
const webdavLastSyncEl = document.getElementById('webdav-last-sync');
const webdavLastStatusEl = document.getElementById('webdav-last-status');

function formatSyncTime(iso) {
    if (!iso) return '从未';
    try {
        const d = new Date(iso);
        if (isNaN(d.getTime())) return iso;
        const pad = n => String(n).padStart(2, '0');
        return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
    } catch { return iso; }
}

// Format the sync-status pill text + color. Takes the full SyncStatus
// object (not just `last_status`) so we can distinguish "not configured"
// (URL/username both None) from "configured but never synced" (last_status
// is None because save_webdav_config clears it on every save, and no sync
// has run yet).
function formatSyncStatus(s) {
    if (!s || !s.configured) {
        return { text: '未配置', color: 'var(--text-muted)' };
    }
    if (!s.last_status) {
        return { text: '已配置,未同步', color: 'var(--text-muted)' };
    }
    const status = s.last_status;
    if (status === 'ok') return { text: '成功', color: '#3fb950' };
    if (status.startsWith('partial')) return { text: status, color: '#d29922' };
    if (status.startsWith('error')) return { text: status, color: '#f85149' };
    return { text: status, color: 'var(--text-dim)' };
}

async function loadWebdavStatus() {
    try {
        const s = await invoke('get_sync_status');
        webdavLastSyncEl.textContent = formatSyncTime(s.last_sync);
        const st = formatSyncStatus(s);
        webdavLastStatusEl.textContent = st.text;
        webdavLastStatusEl.style.color = st.color;
    } catch(e) {
        console.error('Failed to load sync status:', e);
    }
}

webdavSaveBtn.onclick = async () => {
    const url = (webdavUrlEl.value || '').trim();
    const username = (webdavUsernameEl.value || '').trim();
    const password = webdavPasswordEl.value || '';
    if (url && !url.match(/^https?:\/\//)) {
        showToast('WebDAV 地址必须以 http:// 或 https:// 开头', 'error');
        return;
    }
    if (url && !username) {
        showToast('请填写用户名', 'error');
        return;
    }
    try {
        await invoke('save_webdav_config', { url, username, password });
        webdavPasswordEl.value = ''; // Don't keep password in DOM
        showToast('WebDAV 配置已保存(密码已存到系统 keychain)', 'success');
        await loadWebdavStatus();
    } catch(e) {
        showToast(`保存失败: ${e}`, 'error');
    }
};

webdavTestBtn.onclick = async () => {
    const url = (webdavUrlEl.value || '').trim();
    const username = (webdavUsernameEl.value || '').trim();
    if (!url || !username) {
        showToast('请先填写 WebDAV 地址和用户名', 'error');
        return;
    }
    // No auto-save: the test button only verifies the saved config.
    // The separate 保存配置 button is the canonical way to save, and
    // auto-saving here would re-send an empty password (DOM is cleared
    // after 保存 for security) and accidentally delete the keychain entry.
    // If the user hasn't saved yet, the backend will return a friendly
    // "请先点击保存配置" error.
    webdavTestBtn.disabled = true;
    webdavTestBtn.textContent = '测试中...';
    try {
        const result = await invoke('test_webdav_connection');
        showToast(result, 'success');
    } catch(e) {
        showToast(`连接失败: ${e}`, 'error');
    } finally {
        webdavTestBtn.disabled = false;
        webdavTestBtn.textContent = '测试连接';
    }
};

webdavSyncBtn.onclick = async () => {
    webdavSyncBtn.disabled = true;
    const originalText = webdavSyncBtn.textContent;
    webdavSyncBtn.textContent = '同步中...';
    try {
        const result = await invoke('sync_now');
        await loadWebdavStatus();
        if (result === null || result === undefined) {
            // Backend returned null → WebDAV not configured, silent skip
            showToast('WebDAV 未配置,跳过了同步', 'info');
            return;
        }
        if (result.errors && result.errors.length > 0) {
            // 部分完成:只报错误数,完整错误进 console(用户不需要在 toast 里看一堆文件名)
            console.error('WebDAV 同步部分失败:', result);
            showToast(`部分完成 (${result.errors.length} 个错误)`, 'error');
        } else {
            // 成功:toast 极简,详细统计(上传/下载/删除)进 console
            console.log('WebDAV 同步完成:', result);
            if (result.uploaded.length === 0
                && result.downloaded.length === 0
                && result.deleted_remote.length === 0) {
                showToast('同步完成 (无变化)', 'success');
            } else {
                const parts = [];
                if (result.uploaded.length) parts.push(`上传 ${result.uploaded.length} 个`);
                if (result.downloaded.length) parts.push(`下载 ${result.downloaded.length} 个`);
                if (result.deleted_remote.length) parts.push(`删除 ${result.deleted_remote.length} 个`);
                showToast(`同步完成 (${parts.join('，')})`, 'success');
            }
        }
        // 警告不再进 toast(以前会拉一长串);只在 console 留个 log 给调试用
        if (result.warnings && result.warnings.length > 0) {
            console.warn('WebDAV 同步警告:', result.warnings);
        }
    } catch(e) {
        await loadWebdavStatus();
        showToast(`同步失败: ${e}`, 'error');
    } finally {
        webdavSyncBtn.disabled = false;
        webdavSyncBtn.textContent = originalText;
    }
};

// Listen for background WebDAV errors (auto-sync / startup pull / periodic pull)
if (tauri.event && typeof tauri.event.listen === 'function') {
    tauri.event.listen('webdav-error', (event) => {
        const msg = event.payload || '未知错误';
        showToast(`WebDAV: ${msg}`, 'error');
    });
    // 托盘子菜单点击:把对应 profile 切到编辑器
    tauri.event.listen('tray-select-profile', (event) => {
        const id = event.payload;
        if (id) selectProfile(id);
    });
}

document.getElementById('auto-start-checkbox').onchange = async (e) => {
    try {
        await invoke('set_auto_start', { enable: e.target.checked });
        showToast(e.target.checked ? '已开启开机自启' : '已关闭开机自启', 'success');
    } catch(e) {
        e.target.checked = !e.target.checked;
        showToast(`设置失败: ${e}`, 'error');
    }
};

document.getElementsByName('close-behavior').forEach(r => {
    r.onchange = async (e) => {
        const behavior = e.target.value;
        try {
            await invoke('save_close_behavior', { behavior });
            updateRememberGroupVisibility(behavior);
            showToast('已保存关闭行为设置', 'success');
        } catch(e) { console.error(e); }
    };
});

document.getElementById('remember-close-checkbox').onchange = async (e) => {
    try {
        await invoke('save_remember_close_choice', { remember: e.target.checked });
        showToast(e.target.checked ? '已记住本次选择' : '已取消记住', 'success');
    } catch(e) { console.error(e); }
};

// Close Confirm Dialog Logic
const closeConfirmOverlay = document.getElementById('close-confirm-overlay');
const closeToTrayBtn = document.getElementById('close-to-tray-btn');
const closeExitBtn = document.getElementById('close-exit-btn');
const closeRememberCheckbox = document.getElementById('close-remember-checkbox');

function showCloseDialog() {
    closeRememberCheckbox.checked = false;
    closeConfirmOverlay.classList.remove('hidden');
}

function hideCloseDialog() {
    closeConfirmOverlay.classList.add('hidden');
}

closeToTrayBtn.onclick = async () => {
    const remember = closeRememberCheckbox.checked;
    try {
        if (remember) {
            await invoke('save_close_behavior', { behavior: 'tray' });
            await invoke('save_remember_close_choice', { remember: true });
        }
        hideCloseDialog();
        await invoke('hide_to_tray');
    } catch(e) { console.error(e); }
};

closeExitBtn.onclick = async () => {
    const remember = closeRememberCheckbox.checked;
    try {
        if (remember) {
            await invoke('save_close_behavior', { behavior: 'exit' });
            await invoke('save_remember_close_choice', { remember: true });
        }
        hideCloseDialog();
        await invoke('exit_app');
    } catch(e) { console.error(e); }
};

closeConfirmOverlay.onclick = (e) => {
    if (e.target === closeConfirmOverlay) hideCloseDialog();
};

// Init
window.addEventListener('DOMContentLoaded', async () => {
    await initTheme();
    await initWindowSettings();
    await initSidebarWidth();
    await loadData();
    selectProfile('system');

    setupListen();
    if (listen) {
        listen('show-close-dialog', () => {
            // Use the custom in-app modal directly — the system ask() dialog
            // can fail silently (focus/permission issues) and leaves the user
            // thinking the close button is broken.
            showCloseDialog();
        });
    } else {
        console.error('Tauri event.listen is unavailable; close-to-tray dialog will not be wired.');
    }

    setTimeout(() => {
        invoke('show_main_window');
    }, 50);

    // 启动后异步检查更新,有新版就弹窗;不阻塞主流程
    setTimeout(() => { checkUpdateOnStartup(); }, 3000);
});

// =============== 启动更新检查 ===============
async function checkUpdateOnStartup() {
    try {
        // 复用"关于"页的代理设置(localStorage 持久化,默认直连不打扰)
        const { useProxy, proxyBase } = getProxySettings();
        const result = await doCheckUpdate({ useProxy, proxyBase });
        if (!result.found) {
            console.log('Already on latest version');
            return;
        }
        // 弹窗
        const overlay = document.getElementById('update-startup-overlay');
        const curEl = document.getElementById('update-startup-current');
        const latestEl = document.getElementById('update-startup-latest');
        const notesEl = document.getElementById('update-startup-notes');
        const yesBtn = document.getElementById('update-startup-yes');
        const noBtn = document.getElementById('update-startup-no');
        if (!overlay || !yesBtn || !noBtn) return;
        // 拿当前版本号(从 main 进程读)
        let cur = '当前版本';
        try { cur = await invoke('get_app_version'); } catch (_) {}
        curEl.textContent = cur;
        latestEl.textContent = result.version || '—';
        notesEl.textContent = result.notes || '（无更新说明）';
        // 弹窗
        overlay.classList.remove('hidden');
        const dismiss = () => overlay.classList.add('hidden');
        noBtn.onclick = dismiss;
        yesBtn.onclick = async () => {
            yesBtn.disabled = true;
            noBtn.disabled = true;
            const originalYesText = yesBtn.textContent;
            try {
                if (result.mode === 'proxy') {
                    // 走代理:直接用系统默认 handler 打开下载链接
                    yesBtn.textContent = '打开中...';
                    await invoke('hostly_open_url', { url: result.downloadUrl });
                    overlay.classList.add('hidden');
                    showToast('已打开下载链接', 'success');
                } else {
                    // 走直连:原生 downloadAndInstall
                    yesBtn.textContent = '下载中...';
                    await result.update.downloadAndInstall();
                    overlay.classList.add('hidden');
                    showToast('已下载,重启后生效', 'success');
                }
            } catch (e) {
                console.error('Update install failed:', e);
                showToast(`更新失败: ${e}`, 'error');
                yesBtn.disabled = false;
                noBtn.disabled = false;
                yesBtn.textContent = originalYesText;
            }
        };
    } catch (e) {
        // 离线/endpoint 不通/plugin 没初始化 — 都静默吞,不影响主流程
        console.log('Startup update check skipped:', e);
    }
}

// Sidebar Resizing
let isResizingSidebar = false;

async function initSidebarWidth() {
    try {
        const config = await invoke('load_config');
        if (config.sidebar_width) {
            applySidebarWidth(config.sidebar_width);
        }
    } catch(e) { console.error(e); }
}

function applySidebarWidth(w) {
    const sidebar = document.querySelector('.sidebar');
    if (sidebar) sidebar.style.width = w + 'px';
}

const resizer = document.getElementById('sidebar-resizer');
const sidebarEl = document.querySelector('.sidebar');

if (resizer && sidebarEl) {
    resizer.addEventListener('mousedown', (e) => {
        isResizingSidebar = true;
        document.body.style.cursor = 'col-resize';
        e.preventDefault(); 
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizingSidebar) return;
        
        let newWidth = e.clientX;
        if (newWidth < 200) newWidth = 200;
        if (newWidth > 600) newWidth = 600;
        
        sidebarEl.style.width = newWidth + 'px';
    });

    document.addEventListener('mouseup', () => {
        if (isResizingSidebar) {
            isResizingSidebar = false;
            document.body.style.cursor = '';
            // Save persistence
            const w = parseFloat(sidebarEl.style.width);
            invoke('save_sidebar_config', { width: w }).catch(console.error);
        }
    });
}
