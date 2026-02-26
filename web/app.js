const invoke = (command, args) => {
    const tauriInvoke = window.__TAURI__?.core?.invoke;
    if (typeof tauriInvoke !== 'function') {
        return Promise.reject(new Error('Tauri bridge unavailable'));
    }
    return tauriInvoke(command, args);
};

const byId = (id) => document.getElementById(id);
const appRoot = byId('app');

const screens = {
    login: byId('login-screen'),
    hackatime: byId('hackatime-dashboard'),
    adult: byId('adult-dashboard')
};

const elements = {
    loginError: byId('login-error'),
    loginPromo: byId('login-promo'),
    loginUpdateBanner: byId('login-update-banner'),
    loginUpdateText: byId('login-update-text'),
    loginUpdateAction: byId('login-update-action'),
    loginUpdateDismiss: byId('login-update-dismiss'),
    currentProject: byId('current-project'),
    totalHours: byId('total-hours'),
    rpcStatus: byId('rpc-status'),
    rpcDetail: byId('rpc-detail'),
    rpcRefreshButton: byId('btn-rpc-refresh'),
    adultRpcStatus: byId('adult-rpc-status'),
    adultRpcDetail: byId('adult-rpc-detail'),
    adultRpcRefreshButton: byId('btn-adult-rpc-refresh'),
    showReferral: byId('show-referral'),
    showTime: byId('show-time'),
    launchStartup: byId('launch-startup'),
    appEnabled: byId('app-enabled'),
    referralSelect: byId('referral-select'),
    customReferral: byId('custom-referral'),
    referralSection: byId('referral-selection'),
    referralFields: byId('referral-fields'),
    openPyramidButton: byId('btn-open-pyramid'),
    openPyramidAdultButton: byId('btn-open-pyramid-adult'),
    pyramidSignupPromo: byId('pyramid-signup-promo'),
    adultReferralCode: byId('adult-referral-code'),
    adultShowReferral: byId('adult-show-referral'),
    adultReferralSection: byId('adult-referral-section'),
    adultLaunchStartup: byId('adult-launch-startup'),
    adultAppEnabled: byId('adult-app-enabled'),
    confirmModal: byId('confirm-modal'),
    confirmModalMessage: byId('confirm-modal-message'),
    confirmCancel: byId('btn-confirm-cancel'),
    confirmAccept: byId('btn-confirm-accept'),
    apiLoginModal: byId('api-login-modal'),
    apiLoginError: byId('api-login-error'),
    apiLoginInput: byId('api-key-input'),
    apiLoginKeyVisibility: byId('btn-api-key-visibility'),
    apiLoginSubmit: byId('btn-api-login-submit'),
    apiLoginCancel: byId('btn-api-login-cancel'),
    logoutButton: byId('btn-logout'),
    resetButton: byId('btn-reset'),
    dashboardUpdateBanner: byId('dashboard-update-banner'),
    dashboardUpdateText: byId('dashboard-update-text'),
    dashboardUpdateAction: byId('dashboard-update-action'),
    dashboardUpdateDismiss: byId('dashboard-update-dismiss'),
    dashboardFlatpakWarning: byId('dashboard-flatpak-warning'),
    adultUpdateBanner: byId('adult-update-banner'),
    adultUpdateText: byId('adult-update-text'),
    adultUpdateAction: byId('adult-update-action'),
    adultUpdateDismiss: byId('adult-update-dismiss'),
    adultFlatpakWarning: byId('adult-flatpak-warning')
};
const IS_WINDOWS_PLATFORM = document.documentElement.classList.contains('platform-windows');
const IS_LINUX_PLATFORM = document.documentElement.classList.contains('platform-linux');

let confirmResolve = null;
let confirmLastFocused = null;
const PYRAMID_URL = 'https://pyramid.hackclub.com';
const FLAVORTOWN_SETTINGS_URL = 'https://flavortown.hackclub.com/kitchen?settings=1';
const FLAVORTIME_REPO_URL = 'https://github.com/hackclub/flavortime';
const RPC_CONNECTING_MAX_MS = 25000;
const RPC_STATUS_TIMEOUT_MS = 2500;
const RPC_FORCE_REFRESH_TIMEOUT_MS = 5000;
const RPC_RECOVERY_ATTEMPTS = 20;
const RPC_RECOVERY_INTERVAL_MS = 500;
const UPDATER_RECHECK_INTERVAL_MS = 5 * 60 * 1000;
let rpcWaitingAnimationId = null;
let rpcWaitingDots = 1;
let rpcWaitingSinceMs = 0;
let rpcRecoveryRunId = 0;
let authTransitionInProgress = false;
let authFlowRevision = 0;
let updaterBusy = false;
let updaterDownloadDismissed = false;
let updaterDownloadPercent = null;
let updaterProgressUnlisten = null;
let updaterFinishUnlisten = null;
let lastRpcStatus = {
    connected: false,
    enabled: false,
    active: false
};

const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
const UPDATER_ICON_PATHS = Object.freeze({
    refresh: [
        'M20 12a8 8 0 1 1-2.34-5.66',
        'M20 4v5h-5'
    ],
    close: [
        'M6 6L18 18',
        'M18 6L6 18'
    ],
    check: [
        'M5 13L9 17L19 7'
    ]
});

async function openExternal(url) {
    try {
        await invoke('open_external', { url });
    } catch (err) {
        console.error('Failed to open URL:', err);
    }
}

function bindExternalLinks() {
    document.querySelectorAll('.external-link[data-open-url]').forEach((element) => {
        element.addEventListener('click', async (event) => {
            event.preventDefault();
            const url = element.getAttribute('data-open-url');
            if (!url) {
                return;
            }
            await openExternal(url);
        });
    });
}

function updaterTargets() {
    return [
        {
            banner: elements.loginUpdateBanner,
            text: elements.loginUpdateText,
            action: elements.loginUpdateAction,
            dismiss: elements.loginUpdateDismiss
        },
        {
            banner: elements.dashboardUpdateBanner,
            text: elements.dashboardUpdateText,
            action: elements.dashboardUpdateAction,
            dismiss: elements.dashboardUpdateDismiss
        },
        {
            banner: elements.adultUpdateBanner,
            text: elements.adultUpdateText,
            action: elements.adultUpdateAction,
            dismiss: elements.adultUpdateDismiss
        }
    ].filter((target) => target.banner && target.text && target.action && target.dismiss);
}

function flatpakWarningTargets() {
    return [elements.dashboardFlatpakWarning, elements.adultFlatpakWarning].filter(Boolean);
}

function isFlatpakWarningVisible() {
    return flatpakWarningTargets().some((banner) => !banner.classList.contains('hidden'));
}

function updateAttachedBannerLayout() {
    const targets = updaterTargets();
    const updaterVisible = targets.some((target) => !target.banner.classList.contains('hidden'));
    const flatpakVisible = isFlatpakWarningVisible();
    const stacked = updaterVisible && flatpakVisible;

    targets.forEach(({ banner }) => {
        banner.classList.toggle('update-banner-stacked-top', stacked);
    });

    flatpakWarningTargets().forEach((banner) => {
        banner.classList.toggle('update-banner-stacked', stacked);
    });

    setUpdaterButtonExtension(updaterVisible || flatpakVisible);
}

function renderFlatpakWarning(visible) {
    const shouldShow = Boolean(visible);
    flatpakWarningTargets().forEach((banner) => {
        banner.classList.toggle('hidden', !shouldShow);
    });
    updateAttachedBannerLayout();
}

function setUpdaterButtonExtension(active) {
    [elements.logoutButton, elements.resetButton].forEach((button) => {
        if (!button) {
            return;
        }
        button.classList.toggle('update-extended', active);
    });
}

function createUpdaterIcon(name) {
    const svg = document.createElementNS(SVG_NAMESPACE, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('aria-hidden', 'true');
    svg.classList.add('update-banner-icon');

    const paths = UPDATER_ICON_PATHS[name] || [];
    paths.forEach((d) => {
        const path = document.createElementNS(SVG_NAMESPACE, 'path');
        path.setAttribute('d', d);
        svg.append(path);
    });

    return svg;
}

function clearUpdaterButton(button) {
    button.classList.add('hidden');
    button.classList.remove('update-banner-action-icon', 'update-banner-action-with-icon');
    button.replaceChildren();
    button.onclick = null;
    button.disabled = false;
    button.removeAttribute('aria-label');
    button.removeAttribute('title');
}

function setUpdaterButtonHandler(button, onClick) {
    button.onclick = async () => {
        if (button.disabled) {
            return;
        }
        await onClick();
    };
}

function setUpdaterActionButton(action, {
    variant,
    label,
    onClick,
    disabled
}) {
    clearUpdaterButton(action);
    action.classList.remove('hidden');
    setUpdaterButtonHandler(action, onClick);

    if (variant === 'icon-refresh') {
        action.classList.add('update-banner-action-icon');
        action.replaceChildren(createUpdaterIcon('refresh'));
        action.setAttribute('aria-label', label);
        action.setAttribute('title', label);
    } else if (variant === 'text-with-check') {
        action.classList.add('update-banner-action-with-icon');
        const labelTarget = document.createElement('span');
        labelTarget.textContent = label;
        action.replaceChildren(createUpdaterIcon('check'), labelTarget);
    } else {
        action.textContent = label;
    }

    action.disabled = disabled;
}

function setUpdaterDismissButton(dismiss, onClick) {
    const label = t('updater.dismiss_button');
    clearUpdaterButton(dismiss);
    dismiss.classList.remove('hidden');
    dismiss.classList.add('update-banner-action-icon');
    dismiss.replaceChildren(createUpdaterIcon('close'));
    dismiss.disabled = false;
    dismiss.setAttribute('aria-label', label);
    dismiss.setAttribute('title', label);
    setUpdaterButtonHandler(dismiss, onClick);
}

function resetUpdaterButtons(action, dismiss) {
    clearUpdaterButton(action);
    clearUpdaterButton(dismiss);
}

function formatUpdaterMessage(messageKey, tone, progressPercent) {
    const base = t(messageKey);
    if (tone === 'downloading' && Number.isFinite(progressPercent)) {
        return `${base} (${progressPercent}%)`;
    }
    return base;
}

function renderUpdaterState({
    visible,
    tone = null,
    messageKey = null,
    actionKey = null,
    onAction = null,
    actionVariant = 'text',
    showDismiss = false,
    onDismiss = null,
    progressPercent = null
}) {
    const targets = updaterTargets();

    targets.forEach(({ banner }) => {
        banner.classList.toggle('hidden', !visible);
        banner.classList.remove(
            'update-banner-downloading',
            'update-banner-ready',
            'update-banner-failed'
        );
        if (tone) {
            banner.classList.add(`update-banner-${tone}`);
        }
    });

    if (elements.loginPromo) {
        elements.loginPromo.classList.toggle('update-extended', visible);
    }

    if (elements.loginUpdateBanner) {
        elements.loginUpdateBanner.classList.toggle('update-banner-attached-shape', visible);
    }

    updateAttachedBannerLayout();

    if (!visible || !messageKey) {
        targets.forEach(({ action, dismiss }) => {
            resetUpdaterButtons(action, dismiss);
        });
        return;
    }

    const message = formatUpdaterMessage(messageKey, tone, progressPercent);
    targets.forEach(({ text, action, dismiss }) => {
        text.textContent = message;
        resetUpdaterButtons(action, dismiss);

        if (actionKey && onAction) {
            setUpdaterActionButton(action, {
                variant: actionVariant,
                label: t(actionKey),
                onClick: onAction,
                disabled: updaterBusy
            });
        }

        if (showDismiss && onDismiss) {
            setUpdaterDismissButton(dismiss, onDismiss);
        }
    });
}

async function dismissDownloadingUpdaterBanner() {
    updaterDownloadDismissed = true;
    renderUpdaterState({ visible: false });
}

function renderDownloadingUpdaterState() {
    if (updaterDownloadDismissed) {
        return;
    }
    renderUpdaterState({
        visible: true,
        tone: 'downloading',
        messageKey: 'updater.downloading',
        progressPercent: updaterDownloadPercent,
        showDismiss: true,
        onDismiss: dismissDownloadingUpdaterBanner
    });
}

function renderReadyToRestartUpdaterState() {
    renderUpdaterState({
        visible: true,
        tone: 'ready',
        messageKey: 'updater.ready'
    });
}

function renderLinuxManualUpdaterState() {
    renderUpdaterState({
        visible: true,
        tone: 'ready',
        messageKey: 'updater.linux_manual',
        actionKey: 'updater.redownload_button',
        onAction: () => openExternal(FLAVORTIME_REPO_URL),
        showDismiss: true,
        onDismiss: () => renderUpdaterState({ visible: false })
    });
}

async function ensureUpdaterEventListeners() {
    const eventApi = window.__TAURI__?.event;
    if (!eventApi || typeof eventApi.listen !== 'function') {
        return;
    }

    if (!updaterProgressUnlisten) {
        updaterProgressUnlisten = await eventApi.listen(
            'updater-download-progress',
            (event) => {
                const percent = Number(event?.payload);
                if (!Number.isFinite(percent)) {
                    return;
                }
                updaterDownloadPercent = Math.min(100, Math.max(0, Math.round(percent)));
                if (updaterBusy) {
                    renderDownloadingUpdaterState();
                }
            }
        );
    }

    if (!updaterFinishUnlisten) {
        updaterFinishUnlisten = await eventApi.listen('updater-download-finished', () => {
            updaterDownloadPercent = 100;
        });
    }
}

async function restartForUpdate() {
    renderReadyToRestartUpdaterState();

    try {
        await invoke('restart_for_update');
    } catch (err) {
        console.error('Failed to restart for update:', err);
        renderReadyToRestartUpdaterState();
    }
}

async function downloadUpdateAndRender() {
    if (updaterBusy) {
        return;
    }

    updaterBusy = true;
    updaterDownloadDismissed = false;
    updaterDownloadPercent = null;
    renderDownloadingUpdaterState();

    try {
        await invoke('download_update');
        updaterDownloadPercent = 100;

        if (IS_WINDOWS_PLATFORM) {
            updaterBusy = false;
            // Windows updater runs installer + relaunch flow itself; keep this path headless.
            renderUpdaterState({ visible: false });
            return;
        }

        updaterBusy = false;
        renderReadyToRestartUpdaterState();
        void restartForUpdate();
    } catch (err) {
        updaterBusy = false;
        console.error('Update download failed:', err);
        renderUpdaterState({
            visible: true,
            tone: 'failed',
            messageKey: 'updater.failed',
            actionKey: 'updater.retry_button',
            actionVariant: 'icon-refresh',
            showDismiss: true,
            onDismiss: () => renderUpdaterState({ visible: false }),
            onAction: downloadUpdateAndRender
        });
    }
}

async function initUpdaterBanner() {
    try {
        const status = await invoke('check_for_update');
        console.info(
            'Updater check result:',
            JSON.stringify({
                current_version: status?.current_version ?? null,
                available_version: status?.available_version ?? null,
                update_available: Boolean(status?.update_available),
                target: status?.target ?? null,
                dev_mode: Boolean(status?.dev_mode),
                error: status?.error ?? null
            })
        );

        if (status?.error) {
            renderUpdaterState({
                visible: true,
                tone: 'failed',
                messageKey: 'updater.failed',
                actionKey: 'updater.retry_button',
                actionVariant: 'icon-refresh',
                showDismiss: true,
                onDismiss: () => renderUpdaterState({ visible: false }),
                onAction: initUpdaterBanner
            });
            return;
        }

        if (!status?.update_available) {
            renderUpdaterState({ visible: false });
            return;
        }

        if (IS_LINUX_PLATFORM) {
            renderLinuxManualUpdaterState();
            return;
        }

        await downloadUpdateAndRender();
    } catch (err) {
        console.error('Update check failed:', err);
        renderUpdaterState({
            visible: true,
            tone: 'failed',
            messageKey: 'updater.failed',
            actionKey: 'updater.retry_button',
            actionVariant: 'icon-refresh',
            showDismiss: true,
            onDismiss: () => renderUpdaterState({ visible: false }),
            onAction: initUpdaterBanner
        });
    }
}

function setLoginError(message) {
    if (!elements.loginError) {
        return;
    }

    elements.loginError.textContent = message || '';
    elements.loginError.classList.toggle('hidden', !message);
}

function formatLoginError(baseMessage, err) {
    const detail = typeof err === 'string' ? err : err?.message;
    if (!detail) {
        return baseMessage;
    }
    return `${baseMessage}: ${detail}`;
}

function setApiLoginError(message) {
    if (!elements.apiLoginError) {
        return;
    }

    elements.apiLoginError.textContent = message || '';
    elements.apiLoginError.classList.toggle('hidden', !message);
}

function setApiLoginBusy(busy) {
    if (elements.apiLoginSubmit) {
        elements.apiLoginSubmit.disabled = busy;
        elements.apiLoginSubmit.setAttribute('aria-busy', String(busy));
    }
    if (elements.apiLoginCancel) {
        elements.apiLoginCancel.disabled = busy;
    }
    if (elements.apiLoginKeyVisibility) {
        elements.apiLoginKeyVisibility.disabled = busy;
    }
    elements.apiLoginModal?.querySelector('.auth-modal-card')?.classList.toggle('is-loading', busy);
}

function setApiKeyVisibility(visible) {
    if (!elements.apiLoginInput || !elements.apiLoginKeyVisibility) {
        return;
    }

    elements.apiLoginInput.type = visible ? 'text' : 'password';
    elements.apiLoginKeyVisibility.setAttribute('aria-pressed', String(visible));
    const label = visible ? 'Hide API key' : 'Show API key';
    elements.apiLoginKeyVisibility.setAttribute('aria-label', label);
    elements.apiLoginKeyVisibility.title = label;
}

function openApiLoginModal() {
    if (!elements.apiLoginModal) {
        return;
    }

    setApiLoginError('');
    setApiLoginBusy(false);
    if (elements.apiLoginInput) {
        elements.apiLoginInput.value = '';
    }
    setApiKeyVisibility(false);
    elements.apiLoginModal.classList.remove('hidden');
    document.body.classList.add('modal-open');
    elements.apiLoginInput?.focus();
}

function closeApiLoginModal() {
    if (!elements.apiLoginModal) {
        return;
    }

    elements.apiLoginModal.classList.add('hidden');
    setApiKeyVisibility(false);
    document.body.classList.remove('modal-open');
}

function closeConfirmModal(confirmed) {
    if (!confirmResolve || !elements.confirmModal) {
        return;
    }

    const resolve = confirmResolve;
    confirmResolve = null;
    elements.confirmModal.classList.add('hidden');
    document.body.classList.remove('modal-open');

    if (confirmLastFocused && typeof confirmLastFocused.focus === 'function') {
        confirmLastFocused.focus();
    }
    confirmLastFocused = null;

    resolve(confirmed);
}

function promptConfirm(messageKey) {
    if (
        !elements.confirmModal
        || !elements.confirmModalMessage
        || !elements.confirmCancel
        || !elements.confirmAccept
    ) {
        return Promise.resolve(false);
    }

    if (confirmResolve) {
        closeConfirmModal(false);
    }

    return new Promise((resolve) => {
        confirmResolve = resolve;
        confirmLastFocused = document.activeElement;
        elements.confirmModalMessage.textContent = t(messageKey);
        elements.confirmModal.classList.remove('hidden');
        document.body.classList.add('modal-open');
        elements.confirmCancel.focus();
    });
}

function setRpcBadge(target, status) {
    if (!target) {
        return;
    }

    const stalled = Boolean(status.stalled);
    const labelKey = stalled
        ? 'rpc.inactive'
        : (status.active
        ? 'rpc.active'
        : (!status.enabled ? 'rpc.disabled' : (status.connected ? 'rpc.inactive' : 'rpc.waiting')));
    const active = status.active && !stalled;

    target.textContent = t(labelKey);
    target.classList.toggle('rpc-active', active);
    target.classList.toggle('rpc-inactive', !active);
}

function setRpcDetailText(detail) {
    if (elements.rpcDetail) {
        elements.rpcDetail.textContent = detail;
    }
    if (elements.adultRpcDetail) {
        elements.adultRpcDetail.textContent = detail;
    }
}

function isRpcWaiting(status) {
    return Boolean(!status.stalled && status.enabled && !status.connected && !status.active);
}

function clearRpcWaitingAnimation() {
    if (rpcWaitingAnimationId) {
        clearInterval(rpcWaitingAnimationId);
        rpcWaitingAnimationId = null;
    }
}

function renderRpcWaitingDetail() {
    const base = t('rpc.connecting_detail');
    const suffix = '.'.repeat(rpcWaitingDots);
    rpcWaitingDots = (rpcWaitingDots % 3) + 1;
    setRpcDetailText(`${base}${suffix}`);
}

function startRpcWaitingAnimation() {
    if (rpcWaitingAnimationId) {
        return;
    }
    rpcWaitingDots = 1;
    renderRpcWaitingDetail();
    rpcWaitingAnimationId = setInterval(() => {
        renderRpcWaitingDetail();
    }, 320);
}

function wait(ms) {
    return new Promise((resolve) => {
        setTimeout(resolve, ms);
    });
}

async function invokeWithTimeout(command, args, timeoutMs) {
    let timeoutId = null;
    try {
        return await Promise.race([
            invoke(command, args),
            new Promise((_, reject) => {
                timeoutId = setTimeout(() => {
                    reject(new Error(`${command} timed out after ${timeoutMs}ms`));
                }, timeoutMs);
            })
        ]);
    } finally {
        if (timeoutId) {
            clearTimeout(timeoutId);
        }
    }
}

function fallbackRpcStatus({ stalled = false } = {}) {
    return {
        connected: false,
        enabled: lastRpcStatus.enabled,
        active: false,
        flatpak_discord_detected: false,
        stalled: stalled && lastRpcStatus.enabled
    };
}

function setRpcDetail(target, status) {
    if (!target) {
        return;
    }

    const stalled = Boolean(status.stalled);
    const detailKey = stalled
        ? 'rpc.inactive_detail'
        : (status.active
        ? 'rpc.active_detail'
        : (!status.enabled ? 'rpc.disabled_detail' : (status.connected ? 'rpc.inactive_detail' : 'rpc.waiting_detail')));
    const detail = t(detailKey);
    target.textContent = detail;
}

function renderRpcStatus(status) {
    lastRpcStatus = status;
    renderFlatpakWarning(status.flatpak_discord_detected);

    if (isRpcWaiting(status)) {
        if (!rpcWaitingSinceMs) {
            rpcWaitingSinceMs = Date.now();
        }
        const elapsed = Date.now() - rpcWaitingSinceMs;
        if (elapsed >= RPC_CONNECTING_MAX_MS) {
            const stalledStatus = { ...status, stalled: true };
            clearRpcWaitingAnimation();
            setRpcBadge(elements.rpcStatus, stalledStatus);
            setRpcBadge(elements.adultRpcStatus, stalledStatus);
            setRpcDetail(elements.rpcDetail, stalledStatus);
            setRpcDetail(elements.adultRpcDetail, stalledStatus);
            return;
        }

        setRpcBadge(elements.rpcStatus, status);
        setRpcBadge(elements.adultRpcStatus, status);
        startRpcWaitingAnimation();
        return;
    }

    rpcWaitingSinceMs = 0;
    clearRpcWaitingAnimation();
    setRpcBadge(elements.rpcStatus, status);
    setRpcBadge(elements.adultRpcStatus, status);
    setRpcDetail(elements.rpcDetail, status);
    setRpcDetail(elements.adultRpcDetail, status);
}

function setRpcRefreshBusy(busy) {
    [elements.rpcRefreshButton, elements.adultRpcRefreshButton].forEach((button) => {
        if (!button) {
            return;
        }
        button.disabled = busy;
        button.classList.toggle('is-spinning', busy);
    });
}

function isRpcRefreshBusy() {
    return Boolean(elements.rpcRefreshButton?.disabled || elements.adultRpcRefreshButton?.disabled);
}

async function fetchRpcStatus() {
    try {
        return await invokeWithTimeout('get_discord_status', undefined, RPC_STATUS_TIMEOUT_MS);
    } catch (err) {
        console.error('Failed to load Discord status:', err);
        return fallbackRpcStatus({ stalled: true });
    }
}

async function refreshRpcStatus() {
    const status = await fetchRpcStatus();
    renderRpcStatus(status);
    return status;
}

async function fetchForcedRpcStatus() {
    try {
        return await invokeWithTimeout(
            'force_refresh_discord',
            undefined,
            RPC_FORCE_REFRESH_TIMEOUT_MS
        );
    } catch (err) {
        console.error('Failed to force Discord refresh:', err);
        return fallbackRpcStatus();
    }
}

async function recoverRpcStatus({
    forceReconnect = false,
    showBusy = false,
    attempts = RPC_RECOVERY_ATTEMPTS,
    intervalMs = RPC_RECOVERY_INTERVAL_MS
} = {}) {
    if (!showBusy && isRpcRefreshBusy()) {
        return null;
    }

    const runId = ++rpcRecoveryRunId;
    setRpcRefreshBusy(showBusy);

    let status = null;
    try {
        status = forceReconnect
            ? await fetchForcedRpcStatus()
            : await fetchRpcStatus();

        if (runId !== rpcRecoveryRunId) {
            return null;
        }

        renderRpcStatus(status);

        let attempt = 0;
        while (attempt < attempts && isRpcWaiting(status)) {
            attempt += 1;
            await wait(intervalMs);
            if (runId !== rpcRecoveryRunId) {
                return null;
            }

            status = await fetchRpcStatus();
            if (runId !== rpcRecoveryRunId) {
                return null;
            }
            renderRpcStatus(status);
        }

        if (runId !== rpcRecoveryRunId) {
            return null;
        }

        if (isRpcWaiting(status)) {
            const stalledStatus = { ...status, stalled: true };
            renderRpcStatus(stalledStatus);
            status = stalledStatus;
        }

        return status;
    } finally {
        if (runId === rpcRecoveryRunId) {
            setRpcRefreshBusy(false);
        }
    }
}

function forceRefreshRpc() {
    recoverRpcStatus({ forceReconnect: true, showBusy: true });
}

function beginAuthFlowRevision() {
    authFlowRevision += 1;
    return authFlowRevision;
}

function showScreen(screenName) {
    Object.values(screens).forEach((screen) => {
        screen.classList.add('hidden');
    });

    if (screens[screenName]) {
        screens[screenName].classList.remove('hidden');
    }

    if (appRoot) {
        const mergedDashboard = screenName === 'hackatime' || screenName === 'adult';
        const loginMode = screenName === 'login';
        appRoot.classList.toggle('mode-dashboard', mergedDashboard);
        appRoot.classList.toggle('mode-login', loginMode);
    }
}

function pickDefaultReferralCode(codes) {
    if (!codes.length) {
        return null;
    }

    const customCode = codes.find((item) => item.code_type === 'custom');
    return (customCode || codes[0]).code;
}

function updatePyramidSignupPromo(show) {
    if (!elements.pyramidSignupPromo) {
        return;
    }
    elements.pyramidSignupPromo.classList.toggle('hidden', !show);
}

function renderReferralSelect(status) {
    const codes = Array.isArray(status.referral_codes) ? status.referral_codes : [];
    const select = elements.referralSelect;

    select.innerHTML = '';

    if (!codes.length) {
        updatePyramidSignupPromo(true);
        const option = document.createElement('option');
        option.value = '';
        option.textContent = t('referral.no_codes');
        option.selected = true;
        option.disabled = true;
        select.appendChild(option);
        return null;
    }

    updatePyramidSignupPromo(false);
    const availableCodes = new Set(codes.map((item) => item.code));
    const fallbackCode = pickDefaultReferralCode(codes);
    const selectedCode = availableCodes.has(status.selected_referral_code)
        ? status.selected_referral_code
        : fallbackCode;

    codes.forEach((item) => {
        const option = document.createElement('option');
        option.value = item.code;
        option.textContent = item.code;
        option.selected = item.code === selectedCode;
        select.appendChild(option);
    });

    return selectedCode;
}

async function populateSettings(status) {
    const showReferral = status.show_referral_code !== false;
    applyReferralVisibilityState(showReferral);
    elements.showTime.checked = status.show_time_tracking;
    elements.launchStartup.checked = status.launch_at_startup;
    elements.appEnabled.checked = status.app_enabled;

    const selectedCode = renderReferralSelect(status);
    elements.customReferral.value = status.custom_referral_code || '';
    updateReferralSectionState(showReferral);

    if (selectedCode && selectedCode !== status.selected_referral_code) {
        try {
            await invoke('set_selected_referral_code', { code: selectedCode });
        } catch (err) {
            console.error('Failed to persist default referral code:', err);
        }
    }
}

function populateAdultSettings(status) {
    elements.adultReferralCode.value = status.custom_referral_code || '';
    const showReferral = status.show_referral_code !== false;
    applyReferralVisibilityState(showReferral);
    elements.adultLaunchStartup.checked = status.launch_at_startup;
    elements.adultAppEnabled.checked = status.app_enabled;
}

function hasSelectableReferralCodes() {
    return Array.from(elements.referralSelect.options).some((option) => !option.disabled);
}

function updateReferralSectionState(enabled) {
    const shouldDisableSelect = !hasSelectableReferralCodes();
    const hasCustomCode = Boolean(elements.customReferral.value.trim());
    const hiddenInDiscord = !enabled;

    elements.referralSection.classList.toggle('referral-hidden', hiddenInDiscord);

    if (elements.referralFields) {
        elements.referralFields.disabled = false;
    }

    elements.customReferral.disabled = false;
    elements.referralSelect.disabled = shouldDisableSelect || hasCustomCode;
}

function updateAdultReferralSectionState(enabled) {
    const hiddenInDiscord = !enabled;
    elements.adultReferralSection.classList.toggle('referral-hidden', hiddenInDiscord);
    elements.adultReferralCode.disabled = false;
}

function applyReferralVisibilityState(show) {
    elements.showReferral.checked = show;
    elements.adultShowReferral.checked = show;
    updateReferralSectionState(show);
    updateAdultReferralSectionState(show);
}

function formatHours(totalHours) {
    const totalMinutes = Math.round(totalHours * 60);
    const hours = Math.floor(totalMinutes / 60);
    const minutes = totalMinutes % 60;
    return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function setStatLoading(target) {
    if (!target) {
        return;
    }
    target.classList.add('stat-value-loading');
    target.setAttribute('aria-busy', 'true');
    target.textContent = '';
    const loader = document.createElement('span');
    loader.className = 'stat-inline-loader';
    loader.setAttribute('aria-hidden', 'true');
    target.appendChild(loader);
}

function setStatValue(target, value) {
    if (!target) {
        return;
    }
    target.classList.remove('stat-value-loading');
    target.removeAttribute('aria-busy');
    target.textContent = value;
}

function setHackatimeStatsLoading() {
    setStatLoading(elements.currentProject);
    setStatLoading(elements.totalHours);
}

async function loadHackatimeData({ showLoading = false } = {}) {
    if (showLoading) {
        setHackatimeStatsLoading();
    }

    try {
        const data = await invoke('get_hackatime_data');

        setStatValue(elements.currentProject, data.current_project?.name || t('dashboard.stat_empty'));
        setStatValue(elements.totalHours, formatHours(data.total_hours));

        await invoke('update_discord_presence', {
            project: data.current_project?.name || null,
            hours: data.total_hours
        });
    } catch (err) {
        console.error('Failed to load Hackatime data:', err);
        setStatValue(elements.currentProject, t('dashboard.stat_empty'));
        setStatValue(elements.totalHours, t('dashboard.stat_empty'));
    }
}

async function sendFlavortownHeartbeat() {
    try {
        await invoke('send_flavortown_heartbeat');
    } catch (err) {
        console.warn('Flavortown heartbeat failed:', err);
    }
}

async function warmHackatimeData() {
    await loadHackatimeData({ showLoading: true });

    let attempts = 0;
    const intervalId = setInterval(async () => {
        attempts += 1;
        if (attempts >= 6) {
            clearInterval(intervalId);
            return;
        }
        await loadHackatimeData();
    }, 3000);
}

async function showHackatimeDashboard(status = null) {
    const nextStatus = status || await invoke('get_status');
    showScreen('hackatime');
    await populateSettings(nextStatus);
    await warmHackatimeData();
    await invoke('init_discord');
    await recoverRpcStatus();
}

async function initApp() {
    const initRevision = authFlowRevision;

    try {
        await loadLocale('en');
    } catch (err) {
        console.error('Locale load error:', err);
    }

    initUpdaterBanner().catch((err) => {
        console.error('Updater banner init error:', err);
    });

    let status;
    try {
        status = await invoke('get_status');
    } catch (err) {
        console.error('Status load error:', err);
        showScreen('login');
        setLoginError(`Startup error: ${typeof err === 'string' ? err : err?.message || 'unknown'}`);
        return;
    }

    if (authTransitionInProgress || initRevision !== authFlowRevision) {
        return;
    }

    if (status.auth_mode === 'hackatime') {
        try {
            await showHackatimeDashboard(status);
        } catch (err) {
            console.error('Hackatime dashboard init error:', err);
        }
        return;
    }

    if (status.auth_mode === 'adult') {
        showScreen('adult');
        populateAdultSettings(status);
        try {
            await invoke('init_discord');
        } catch (err) {
            console.error('Discord init error (non-fatal):', err);
        }
        try {
            await recoverRpcStatus();
        } catch (err) {
            console.error('RPC recovery error:', err);
        }
        return;
    }

    showScreen('login');
}

byId('btn-hackatime-login').addEventListener('click', () => {
    openApiLoginModal();
    void openExternal(FLAVORTOWN_SETTINGS_URL);
});

if (elements.apiLoginCancel) {
    elements.apiLoginCancel.addEventListener('click', () => {
        closeApiLoginModal();
    });
}

if (elements.apiLoginModal) {
    elements.apiLoginModal.addEventListener('click', (event) => {
        if (event.target === elements.apiLoginModal) {
            closeApiLoginModal();
        }
    });
}

if (elements.apiLoginKeyVisibility) {
    elements.apiLoginKeyVisibility.addEventListener('click', () => {
        const currentlyVisible = elements.apiLoginInput?.type === 'text';
        setApiKeyVisibility(!currentlyVisible);
        elements.apiLoginInput?.focus();
    });
}

if (elements.apiLoginSubmit) {
    elements.apiLoginSubmit.addEventListener('click', async () => {
        const apiKey = (elements.apiLoginInput?.value || '').trim();
        if (!apiKey) {
            setApiLoginError('Please enter your Flavortown API key.');
            return;
        }

        const loginRevision = beginAuthFlowRevision();
        authTransitionInProgress = true;
        setApiLoginBusy(true);
        setApiLoginError('');

        try {
            await invoke('login_with_flavortown_api_key', { apiKey });
            if (loginRevision !== authFlowRevision) {
                return;
            }

            setLoginError('');
            closeApiLoginModal();
            const status = await invoke('get_status');
            await showHackatimeDashboard(status);
        } catch (err) {
            console.error('Flavortime login failed:', err);
            setApiLoginError(formatLoginError('Login failed', err));
        } finally {
            authTransitionInProgress = false;
            setApiLoginBusy(false);
        }
    });
}

if (elements.apiLoginInput) {
    elements.apiLoginInput.addEventListener('keydown', async (event) => {
        if (event.key === 'Enter') {
            event.preventDefault();
            elements.apiLoginSubmit?.click();
        }
    });
}

byId('btn-adult-login').addEventListener('click', async () => {
    if (authTransitionInProgress) {
        return;
    }

    beginAuthFlowRevision();
    authTransitionInProgress = true;
    try {
        setLoginError('');
        await invoke('login_as_adult');
        await invoke('init_discord');
        const status = await invoke('get_status');
        showScreen('adult');
        populateAdultSettings(status);
        await recoverRpcStatus();
    } catch (err) {
        console.error('Adult login error:', err);
    } finally {
        authTransitionInProgress = false;
    }
});

elements.showReferral.addEventListener('change', (event) => {
    const show = event.target.checked;
    applyReferralVisibilityState(show);
    invoke('set_show_referral_code', { show }).catch((err) => {
        console.error('Error:', err);
        applyReferralVisibilityState(!show);
    });
});

elements.referralSelect.addEventListener('change', async (event) => {
    try {
        const code = event.target.value || null;
        await invoke('set_selected_referral_code', { code });
    } catch (err) {
        console.error('Error:', err);
    }
});

elements.customReferral.addEventListener('change', async (event) => {
    try {
        const code = event.target.value.trim() || null;
        await invoke('set_custom_referral_code', { code });
        updateReferralSectionState(elements.showReferral.checked);
    } catch (err) {
        console.error('Error:', err);
    }
});

elements.customReferral.addEventListener('input', () => {
    updateReferralSectionState(elements.showReferral.checked);
});

elements.showTime.addEventListener('change', (event) => {
    const show = event.target.checked;
    invoke('set_show_time_tracking', { show })
        .then(() => {
            if (!screens.hackatime.classList.contains('hidden')) {
                loadHackatimeData({ showLoading: true });
            }
        })
        .catch((err) => {
            console.error('Error:', err);
            event.target.checked = !show;
        });
});

elements.launchStartup.addEventListener('change', (event) => {
    const enabled = event.target.checked;
    invoke('set_launch_at_startup', { enabled }).catch((err) => {
        console.error('Error:', err);
        event.target.checked = !enabled;
    });
});

elements.appEnabled.addEventListener('change', (event) => {
    const enabled = event.target.checked;
    invoke('set_app_enabled', { enabled })
        .catch((err) => {
            console.error('Error:', err);
            event.target.checked = !enabled;
        })
        .finally(() => {
            if (enabled) {
                recoverRpcStatus();
            } else {
                refreshRpcStatus();
            }
        });
});

if (elements.rpcRefreshButton) {
    elements.rpcRefreshButton.addEventListener('click', () => {
        forceRefreshRpc();
    });
}

if (elements.adultRpcRefreshButton) {
    elements.adultRpcRefreshButton.addEventListener('click', () => {
        forceRefreshRpc();
    });
}

byId('btn-logout').addEventListener('click', async () => {
    const confirmed = await promptConfirm('actions.confirm_logout');
    if (!confirmed) {
        return;
    }

    try {
        await invoke('logout');
        await refreshRpcStatus();
        showScreen('login');
    } catch (err) {
        console.error('Logout error:', err);
    }
});

elements.adultShowReferral.addEventListener('change', (event) => {
    const show = event.target.checked;
    applyReferralVisibilityState(show);
    invoke('set_show_referral_code', { show }).catch((err) => {
        console.error('Error:', err);
        applyReferralVisibilityState(!show);
    });
});

elements.adultReferralCode.addEventListener('change', async (event) => {
    try {
        const code = event.target.value.trim();
        await invoke('set_adult_referral_code', { code });
        updateAdultReferralSectionState(elements.adultShowReferral.checked);
    } catch (err) {
        console.error('Error:', err);
    }
});

elements.adultReferralCode.addEventListener('input', () => {
    updateAdultReferralSectionState(elements.adultShowReferral.checked);
});

elements.adultLaunchStartup.addEventListener('change', (event) => {
    const enabled = event.target.checked;
    invoke('set_launch_at_startup', { enabled }).catch((err) => {
        console.error('Error:', err);
        event.target.checked = !enabled;
    });
});

elements.adultAppEnabled.addEventListener('change', (event) => {
    const enabled = event.target.checked;
    invoke('set_app_enabled', { enabled })
        .catch((err) => {
            console.error('Error:', err);
            event.target.checked = !enabled;
        })
        .finally(() => {
            if (enabled) {
                recoverRpcStatus();
            } else {
                refreshRpcStatus();
            }
        });
});

byId('btn-reset').addEventListener('click', async () => {
    const confirmed = await promptConfirm('actions.confirm_reset');
    if (!confirmed) {
        return;
    }

    try {
        await invoke('logout');
        await refreshRpcStatus();
        showScreen('login');
    } catch (err) {
        console.error('Reset error:', err);
    }
});

function bindPyramidButton(button) {
    if (!button) {
        return;
    }
    button.addEventListener('click', async () => {
        await openExternal(PYRAMID_URL);
    });
}

bindPyramidButton(elements.openPyramidButton);
bindPyramidButton(elements.openPyramidAdultButton);

if (elements.confirmCancel) {
    elements.confirmCancel.addEventListener('click', () => {
        closeConfirmModal(false);
    });
}

if (elements.confirmAccept) {
    elements.confirmAccept.addEventListener('click', () => {
        closeConfirmModal(true);
    });
}

if (elements.confirmModal) {
    elements.confirmModal.addEventListener('click', (event) => {
        if (event.target === elements.confirmModal) {
            closeConfirmModal(false);
        }
    });
}

document.addEventListener('keydown', (event) => {
    const apiLoginModalOpen = elements.apiLoginModal && !elements.apiLoginModal.classList.contains('hidden');
    if (apiLoginModalOpen && event.key === 'Escape') {
        event.preventDefault();
        closeApiLoginModal();
        return;
    }

    if (!confirmResolve) {
        return;
    }

    if (event.key === 'Escape') {
        event.preventDefault();
        closeConfirmModal(false);
        return;
    }

    if (event.key === 'Enter' && document.activeElement !== elements.confirmCancel) {
        event.preventDefault();
        closeConfirmModal(true);
    }
});

bindExternalLinks();

async function minuteTick() {
    try {
        const status = await invoke('get_status');
        if (status.auth_mode === 'hackatime') {
            await loadHackatimeData();
        }
    } catch (err) {
        console.error('Refresh error:', err);
    }

    await sendFlavortownHeartbeat();
}

setInterval(minuteTick, 60000);

setInterval(() => {
    recoverRpcStatus({
        attempts: 4,
        intervalMs: 400
    });
}, 20000);

setInterval(() => {
    if (!updaterBusy) {
        initUpdaterBanner().catch((err) => {
            console.error('Scheduled updater check failed:', err);
        });
    }
}, UPDATER_RECHECK_INTERVAL_MS);

initApp();
minuteTick();
