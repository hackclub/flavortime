let strings = {};

function resolve(key) {
    const parts = key.split('.');
    let value = strings;
    for (const part of parts) {
        if (value == null || typeof value !== 'object') {
            return key;
        }
        value = value[part];
    }
    return typeof value === 'string' ? value : key;
}

function t(key) {
    return resolve(key);
}

function applyStaticStrings() {
    document.querySelectorAll('[data-i18n]').forEach((el) => {
        el.textContent = t(el.getAttribute('data-i18n'));
    });

    document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
        el.placeholder = t(el.getAttribute('data-i18n-placeholder'));
    });

    document.querySelectorAll('[data-i18n-title]').forEach((el) => {
        el.title = t(el.getAttribute('data-i18n-title'));
    });

    document.querySelectorAll('[data-i18n-html]').forEach((el) => {
        const key = el.getAttribute('data-i18n-html');
        el.innerHTML = t(key);
    });
}

async function loadLocale(locale) {
    try {
        const response = await fetch(`locales/${locale}.json`);
        strings = await response.json();
    } catch (err) {
        console.error('Failed to load locale:', locale, err);
    }
    applyStaticStrings();
}
