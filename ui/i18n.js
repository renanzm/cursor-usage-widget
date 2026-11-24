// Textos da interface em inglês e português. O idioma vem do Rust (`get_lang`), que resolve "Automático".
// Marcação no HTML: data-i18n (texto), data-i18n-html, data-i18n-title, data-i18n-placeholder, data-i18n-aria.
(() => {
  const STR = {
    en: {
      // widget
      pace: "Pace",
      settings: "Settings",
      last: "last",
      session: "Session",
      week: "Week",
      period: "Period",
      monthlyCycle: "Monthly cycle",
      weekDays: "Week · Mon–Fri",
      connectCursor: "Connect to Cursor",
      pasteCookie: "Paste the <code>WorkosCursorSessionToken</code> cookie",
      cookiePlaceholder: "Paste the cookie here",
      save: "Save",
      days: ["Mon", "Tue", "Wed", "Thu", "Fri"],
      levels: { low: "Low", mid: "Medium", high: "High" },
      resetDone: "window already reset",
      resetsIn: "resets in {t}",
      lastRead: "Last reading {t} ago (updates while {app} is open)",
      waiting: "Waiting for {app} (open a session and send a message)",
      tabCycle: "cycle",
      tabWeek: "week",
      sourceTitle: "Source: {s}",
      of: "of {v}",
      projection: "Projection {v} · {d}d left",
      // configurações
      settingsTitle: "Settings · Cursor Usage",
      dataSource: "Data source",
      automatic: "Automatic",
      found: "logs found",
      notFound: "not found",
      cookieSaved: "cookie saved",
      cookieNeeded: "asks for the cookie in the widget",
      uses: "uses {s}",
      monthlyQuota: "Monthly quota",
      quotaAuto: "Automatic (from Cursor)",
      manual: "Manual",
      quotaValue: "Quota amount (USD)",
      windowHours: "Calculation window (hours)",
      rateMax: "Speedometer end of scale (USD/h)",
      lowMax: "Low up to (USD/h)",
      highMin: "High from (USD/h)",
      dayMax: "Equalizer daily scale (USD)",
      general: "General",
      pollSeconds: "Refresh every (seconds)",
      position: "Position",
      monitor: "Monitor",
      monitorAuto: "Automatic (rightmost)",
      disconnected: "Disconnected ({m})",
      left: "Left side",
      right: "Right side",
      appearance: "Appearance",
      dark: "Dark",
      light: "Light",
      language: "Language",
      langAuto: "Automatic (Windows)",
      colors: "Colors",
      accent: "Accent",
      low: "Low",
      mid: "Medium",
      high: "High",
      resetColors: "Restore default colors",
      resetAll: "Restore all",
      cancel: "Cancel",
      enterNumber: "Enter a number.",
      fixFields: "Fix the highlighted fields.",
    },
    pt: {
      pace: "Ritmo",
      settings: "Configurações",
      last: "últimas",
      session: "Sessão",
      week: "Semana",
      period: "Período",
      monthlyCycle: "Ciclo mensal",
      weekDays: "Semana · seg–sex",
      connectCursor: "Conectar ao Cursor",
      pasteCookie: "Cole o cookie <code>WorkosCursorSessionToken</code>",
      cookiePlaceholder: "Cole o cookie aqui",
      save: "Salvar",
      days: ["Seg", "Ter", "Qua", "Qui", "Sex"],
      levels: { low: "Baixo", mid: "Médio", high: "Alto" },
      resetDone: "janela já resetou",
      resetsIn: "reseta em {t}",
      lastRead: "Última leitura há {t} (atualiza com o {app} aberto)",
      waiting: "Aguardando o {app} (abra uma sessão e envie uma mensagem)",
      tabCycle: "ciclo",
      tabWeek: "semana",
      sourceTitle: "Fonte: {s}",
      of: "de {v}",
      projection: "Projeção {v} · {d}d restantes",
      settingsTitle: "Configurações · Cursor Usage",
      dataSource: "Fonte de dados",
      automatic: "Automático",
      found: "logs encontrados",
      notFound: "não encontrado",
      cookieSaved: "cookie salvo",
      cookieNeeded: "pede o cookie no widget",
      uses: "usa {s}",
      monthlyQuota: "Cota mensal",
      quotaAuto: "Automático (do Cursor)",
      manual: "Manual",
      quotaValue: "Valor da cota (US$)",
      windowHours: "Janela de cálculo (horas)",
      rateMax: "Fim da escala do velocímetro (US$/h)",
      lowMax: "Baixo até (US$/h)",
      highMin: "Alto a partir de (US$/h)",
      dayMax: "Escala diária do equalizador (US$)",
      general: "Geral",
      pollSeconds: "Atualizar a cada (segundos)",
      position: "Posição",
      monitor: "Monitor",
      monitorAuto: "Automático (o mais à direita)",
      disconnected: "Desconectado ({m})",
      left: "Lado esquerdo",
      right: "Lado direito",
      appearance: "Aparência",
      dark: "Escuro",
      light: "Claro",
      language: "Idioma",
      langAuto: "Automático (Windows)",
      colors: "Cores",
      accent: "Destaque",
      low: "Baixo",
      mid: "Médio",
      high: "Alto",
      resetColors: "Restaurar cores padrão",
      resetAll: "Restaurar tudo",
      cancel: "Cancelar",
      enterNumber: "Informe um número.",
      fixFields: "Corrija os campos destacados.",
    },
  };

  let lang = "en";
  const I18N = {
    get lang() {
      return lang;
    },
    get locale() {
      return lang === "pt" ? "pt-BR" : "en-US";
    },
    /** Texto da chave, com {variáveis} substituídas. */
    t(key, vars = {}) {
      const v = STR[lang][key] ?? STR.en[key] ?? key;
      return typeof v === "string" ? v.replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? "") : v;
    },
    /** Troca o idioma e reescreve os textos marcados no HTML. */
    set(code) {
      lang = STR[code] ? code : "en";
      document.documentElement.lang = I18N.locale;
      const each = (attr, fn) => document.querySelectorAll(`[${attr}]`).forEach((el) => fn(el, I18N.t(el.getAttribute(attr))));
      each("data-i18n", (el, s) => (el.textContent = s));
      each("data-i18n-html", (el, s) => (el.innerHTML = s));
      each("data-i18n-title", (el, s) => (el.title = s));
      each("data-i18n-placeholder", (el, s) => (el.placeholder = s));
      each("data-i18n-aria", (el, s) => el.setAttribute("aria-label", s));
      const title = document.querySelector("title[data-i18n]");
      if (title) document.title = title.textContent;
    },
  };
  window.I18N = I18N;
})();
