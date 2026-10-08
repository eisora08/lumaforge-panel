import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react';

export type Lang = 'en' | 'es';

export type TranslateArg = string | number;

// ---------------------------------------------------------------------------
// Catalogs
// ---------------------------------------------------------------------------

const EN: Record<string, string> = {
  // shared action buttons
  'ui.update': 'UPDATE',
  'ui.install': 'INSTALL',
  'ui.working': 'WORKING...',
  'ui.done': 'DONE',
  'ui.refresh': 'Refresh',

  // window / titlebar
  'app.updatesBadge': '{n} updates available',
  'app.openDataFolder': 'Open LumaForge data folder',
  'app.settings': 'Settings',
  'app.minimize': 'Minimize window',
  'app.close': 'Close window',

  // settings menu
  'menu.updates': 'Updates',
  'menu.checking': 'Checking for updates...',
  'menu.check': 'Check for updates',
  'menu.nAvailable': '{n} available',
  'menu.upToDate': 'Up to date',
  'menu.startup': 'Startup',
  'menu.startAuto': 'Start automatically',
  'menu.startMin': 'Start minimized to tray',
  'menu.closeTray': 'Close button hides to tray',
  'menu.appearance': 'Appearance',
  'menu.theme': 'Theme',
  'menu.language': 'Language',

  // updates menu
  'updates.available': 'Updates available',
  'updates.newRelease': 'new release',
  'updates.downloading': 'Downloading...',
  'updates.installing': 'Installing...',
  'toast.updateAvailable': '{n} update available.',
  'toast.updatesAvailable': '{n} updates available.',
  'toast.upToDate': 'Everything is up to date.',
  'toast.restartForUpdate': 'Restart Steam to finish applying the update.',

  // close confirm
  'close.title': 'Close LumaForge Panel',
  'close.desc':
    'Minimize to the system tray to keep the panel handy, or close the application completely?',
  'close.confirm': 'CLOSE APP',
  'close.cancel': 'MINIMIZE TO TRAY',

  // dashboard — component badges
  'badge.active': 'Active',
  'badge.backedUp': 'Backed up',
  'badge.notInstalled': 'Not installed',

  // dashboard — hero states
  'hero.busyEyebrow': 'APPLYING CHANGES',
  'hero.busyTitle': 'Updating CDP injection…',
  'hero.busyDesc': 'Steam may restart to apply the new state.',
  'hero.busyBtn': 'WAIT',
  'hero.busyDesc2': 'Operation in progress',
  'hero.checkEyebrow': 'CHECKING SYSTEM',
  'hero.checkTitle': 'Verifying system state',
  'hero.checkDesc': 'Reading components from disk.',
  'hero.checkBtn': 'CHECKING',
  'hero.checkDesc2': 'Reading system state',
  'hero.missingEyebrow': 'RUNTIME NOT INSTALLED',
  'hero.missingTitle': 'Install the LumaForge runtime',
  'hero.missingDesc': '{loader} is not present in the Steam directory yet.',
  'hero.missingBtn': 'INSTALL',
  'hero.missingDesc2': 'Download and install the runtime',
  'hero.enabledEyebrow': 'CDP INJECTION ACTIVE',
  'hero.enabledTitle': 'LumaForge is running',
  'hero.enabledDesc':
    '{loader} is loaded by Steam and extensions are being injected.',
  'hero.enabledBtn': 'ENABLED',
  'hero.enabledDesc2': 'Disable CDP injection',
  'hero.disabledEyebrow': 'CDP INJECTION OFF',
  'hero.disabledTitle': 'LumaForge is paused',
  'hero.disabledDesc':
    'The loader is backed up as {loader}.bak and Steam loads vanilla.',
  'hero.disabledBtn': 'DISABLED',
  'hero.disabledDesc2': 'Enable CDP injection',

  // dashboard — hero facts
  'steam.checking': 'Checking',
  'steam.running': 'Running',
  'steam.stopped': 'Stopped',
  'steam.startSteam': 'Start Steam',
  'fact.components': 'Components:',
  'fact.componentsOn': 'Components on:',
  'fact.count': '{a} of {b}',

  // dashboard — aria
  'aria.cdpInjection': 'CDP injection',
  'aria.components': 'Components',
  'aria.disableCdp': 'Disable CDP injection',
  'aria.enableCdp': 'Enable CDP injection',
  'aria.uninstall': 'Uninstall {name}',
  'aria.toggleTool': 'Toggle {name}',

  // dashboard — footer
  'footer.steamRoot': 'Steam · {path}',
  'footer.noSteamRoot': 'Steam root not detected',

  // restart modal
  'restart.title': 'Restart Steam to apply changes?',
  'restart.desc':
    'Steam must restart so it can reload the patched DLLs and pick up the new component state.',
  'restart.warning':
    'Make sure no game, installation, or download is currently active before restarting Steam.',
  'restart.confirm': 'RESTART STEAM',
  'restart.cancel': 'LATER',
  'restart.busy': 'RESTARTING...',

  // install modal
  'install.complete': 'Installation complete',
  'install.failed': 'Installation failed',
  'install.runtimeTitle': 'Install LumaForge runtime',
  'install.toolTitle': 'Install {name}',
  'install.doneMsg': 'Done.',
  'install.wrong': 'Something went wrong.',
  'install.desc':
    'Downloads the release from GitHub, extracts it and deploys the files.',

  // uninstall modal
  'uninstall.title': 'Uninstall {name}?',
  'uninstall.desc':
    'Removes the files deployed to {path} and clears the install state. You can install it again later.',
  'uninstall.warning':
    'Make sure Steam is not running — it will be closed if needed so the files can be removed.',
  'uninstall.confirm': 'UNINSTALL',
  'uninstall.cancel': 'CANCEL',
  'uninstall.busy': 'REMOVING...',
  'uninstall.disk': 'disk',

  // themes card
  'themes.title': 'Themes',
  'themes.hint': ' — restart Steam if it doesn’t look right',
  'themes.noInstalled': 'No themes installed',
  'themes.loadingTag': 'LOADING',
  'themes.offTag': 'OFF',
  'themes.count': '{n} installed',
  'themes.activeBtn': 'ACTIVE',
  'themes.applyBtn': 'APPLY',
  'themes.cdpNote':
    'CDP injection is off — enable it so theme changes reach Steam live.',
  'themes.loadingMsg': 'Loading themes…',
  'themes.emptyBefore': 'No Steam themes found. Install themes into',
  'themes.emptyAfter': '.',
  'themes.conditionsLabel': 'Conditions',
  'themes.activeBadge': 'Active',

  // themes — settings panel
  'panel.activeTheme': 'Active theme',
  'panel.settings': 'Theme settings',
  'panel.conditionsCount': ' · {n} conditions',
  'panel.noConditions': 'This theme has no configurable conditions.',
  'panel.footer':
    'Changes apply live — restart Steam if it doesn’t look right.',
  'panel.closeEsc': 'Close (Esc)',

  // themes — aria
  'aria.themeSettings': 'Theme settings',
  'aria.refreshThemes': 'Refresh themes',
  'aria.disableTheming': 'Disable theming',
  'aria.enableTheming': 'Enable theming',
  'aria.scrollLeft': 'Scroll themes left',
  'aria.scrollRight': 'Scroll themes right',
  'aria.closeThemeSettings': 'Close theme settings',
  'aria.resetCondition': 'Reset {key}',
  'resetToDefault': 'Reset to theme default',

  // confirm modal defaults
  'modal.confirm': 'CONFIRM',
  'modal.cancel': 'CANCEL',
  'modal.pleaseWait': 'PLEASE WAIT...',
  'aria.closeConfirmation': 'Close confirmation',

  // toast
  'aria.notifications': 'Notifications',
  'aria.dismissNotification': 'Dismiss notification',
};

const ES: Record<string, string> = {
  'ui.update': 'ACTUALIZAR',
  'ui.install': 'INSTALAR',
  'ui.working': 'PROCESANDO...',
  'ui.done': 'LISTO',
  'ui.refresh': 'Actualizar',

  'app.updatesBadge': '{n} actualizaciones disponibles',
  'app.openDataFolder': 'Abrir carpeta de datos de LumaForge',
  'app.settings': 'Ajustes',
  'app.minimize': 'Minimizar ventana',
  'app.close': 'Cerrar ventana',

  'menu.updates': 'Actualizaciones',
  'menu.checking': 'Buscando actualizaciones...',
  'menu.check': 'Buscar actualizaciones',
  'menu.nAvailable': '{n} disponibles',
  'menu.upToDate': 'Todo actualizado',
  'menu.startup': 'Inicio',
  'menu.startAuto': 'Iniciar automáticamente',
  'menu.startMin': 'Iniciar minimizado en la bandeja',
  'menu.closeTray': 'El botón cerrar minimiza a la bandeja',
  'menu.appearance': 'Apariencia',
  'menu.theme': 'Tema',
  'menu.language': 'Idioma',

  'updates.available': 'Actualizaciones disponibles',
  'updates.newRelease': 'nueva versión',
  'updates.downloading': 'Descargando...',
  'updates.installing': 'Instalando...',
  'toast.updateAvailable': '{n} actualización disponible.',
  'toast.updatesAvailable': '{n} actualizaciones disponibles.',
  'toast.upToDate': 'Todo está actualizado.',
  'toast.restartForUpdate':
    'Reinicia Steam para terminar de aplicar la actualización.',

  'close.title': 'Cerrar LumaForge Panel',
  'close.desc':
    'Minimiza a la bandeja del sistema para tener el panel a mano, o cierra la aplicación por completo.',
  'close.confirm': 'CERRAR APP',
  'close.cancel': 'MINIMIZAR A LA BANDEJA',

  'badge.active': 'Activo',
  'badge.backedUp': 'Respaldado',
  'badge.notInstalled': 'No instalado',

  'hero.busyEyebrow': 'APLICANDO CAMBIOS',
  'hero.busyTitle': 'Actualizando la inyección CDP…',
  'hero.busyDesc': 'Steam puede reiniciarse para aplicar el nuevo estado.',
  'hero.busyBtn': 'ESPERAR',
  'hero.busyDesc2': 'Operación en curso',
  'hero.checkEyebrow': 'VERIFICANDO SISTEMA',
  'hero.checkTitle': 'Verificando el estado del sistema',
  'hero.checkDesc': 'Leyendo componentes del disco.',
  'hero.checkBtn': 'VERIFICANDO',
  'hero.checkDesc2': 'Leyendo el estado del sistema',
  'hero.missingEyebrow': 'RUNTIME NO INSTALADO',
  'hero.missingTitle': 'Instala el runtime de LumaForge',
  'hero.missingDesc':
    '{loader} aún no está presente en el directorio de Steam.',
  'hero.missingBtn': 'INSTALAR',
  'hero.missingDesc2': 'Descargar e instalar el runtime',
  'hero.enabledEyebrow': 'INYECCIÓN CDP ACTIVA',
  'hero.enabledTitle': 'LumaForge está en ejecución',
  'hero.enabledDesc':
    'Steam carga {loader} y las extensiones se están inyectando.',
  'hero.enabledBtn': 'ACTIVADO',
  'hero.enabledDesc2': 'Desactivar la inyección CDP',
  'hero.disabledEyebrow': 'INYECCIÓN CDP DESACTIVADA',
  'hero.disabledTitle': 'LumaForge está en pausa',
  'hero.disabledDesc':
    'El loader está respaldado como {loader}.bak y Steam carga la versión original.',
  'hero.disabledBtn': 'DESACTIVADO',
  'hero.disabledDesc2': 'Activar la inyección CDP',

  'steam.checking': 'Verificando',
  'steam.running': 'En ejecución',
  'steam.stopped': 'Detenido',
  'steam.startSteam': 'Iniciar Steam',
  'fact.components': 'Componentes:',
  'fact.componentsOn': 'Componentes activos:',
  'fact.count': '{a} de {b}',

  'aria.cdpInjection': 'Inyección CDP',
  'aria.components': 'Componentes',
  'aria.disableCdp': 'Desactivar la inyección CDP',
  'aria.enableCdp': 'Activar la inyección CDP',
  'aria.uninstall': 'Desinstalar {name}',
  'aria.toggleTool': 'Conmutar {name}',

  'footer.steamRoot': 'Steam · {path}',
  'footer.noSteamRoot': 'Raíz de Steam no detectada',

  'restart.title': '¿Reiniciar Steam para aplicar los cambios?',
  'restart.desc':
    'Steam debe reiniciarse para recargar las DLL parcheadas y adoptar el nuevo estado de los componentes.',
  'restart.warning':
    'Asegúrate de que no haya ninguna partida, instalación o descarga en curso antes de reiniciar Steam.',
  'restart.confirm': 'REINICIAR STEAM',
  'restart.cancel': 'MÁS TARDE',
  'restart.busy': 'REINICIANDO...',

  'install.complete': 'Instalación completada',
  'install.failed': 'La instalación falló',
  'install.runtimeTitle': 'Instalar el runtime de LumaForge',
  'install.toolTitle': 'Instalar {name}',
  'install.doneMsg': 'Listo.',
  'install.wrong': 'Algo salió mal.',
  'install.desc':
    'Descarga la versión desde GitHub, la extrae y despliega los archivos.',

  'uninstall.title': '¿Desinstalar {name}?',
  'uninstall.desc':
    'Elimina los archivos desplegados en {path} y borra el estado de instalación. Puedes instalarlo de nuevo más tarde.',
  'uninstall.warning':
    'Asegúrate de que Steam no esté en ejecución: se cerrará si hace falta para poder eliminar los archivos.',
  'uninstall.confirm': 'DESINSTALAR',
  'uninstall.cancel': 'CANCELAR',
  'uninstall.busy': 'ELIMINANDO...',
  'uninstall.disk': 'disco',

  'themes.title': 'Temas',
  'themes.hint': ' — reinicia Steam si algo no se ve bien',
  'themes.noInstalled': 'No hay temas instalados',
  'themes.loadingTag': 'CARGANDO',
  'themes.offTag': 'APAGADO',
  'themes.count': '{n} instalados',
  'themes.activeBtn': 'ACTIVO',
  'themes.applyBtn': 'APLICAR',
  'themes.cdpNote':
    'La inyección CDP está desactivada: actívala para que los cambios de tema lleguen a Steam en vivo.',
  'themes.loadingMsg': 'Cargando temas…',
  'themes.emptyBefore': 'No se encontraron temas de Steam. Instala temas en',
  'themes.emptyAfter': '.',
  'themes.conditionsLabel': 'Condiciones',
  'themes.activeBadge': 'Activo',

  'panel.activeTheme': 'Tema activo',
  'panel.settings': 'Ajustes del tema',
  'panel.conditionsCount': ' · {n} condiciones',
  'panel.noConditions': 'Este tema no tiene condiciones configurables.',
  'panel.footer':
    'Los cambios se aplican en vivo: reinicia Steam si algo no se ve bien.',
  'panel.closeEsc': 'Cerrar (Esc)',

  'aria.themeSettings': 'Ajustes del tema',
  'aria.refreshThemes': 'Actualizar temas',
  'aria.disableTheming': 'Desactivar temas',
  'aria.enableTheming': 'Activar temas',
  'aria.scrollLeft': 'Desplazar temas a la izquierda',
  'aria.scrollRight': 'Desplazar temas a la derecha',
  'aria.closeThemeSettings': 'Cerrar ajustes del tema',
  'aria.resetCondition': 'Restablecer {key}',
  'resetToDefault': 'Restablecer al valor del tema',

  'modal.confirm': 'CONFIRMAR',
  'modal.cancel': 'CANCELAR',
  'modal.pleaseWait': 'ESPERA POR FAVOR...',
  'aria.closeConfirmation': 'Cerrar confirmación',

  'aria.notifications': 'Notificaciones',
  'aria.dismissNotification': 'Descartar notificación',
};

const CATALOGS: Record<Lang, Record<string, string>> = { en: EN, es: ES };

/**
 * Replace each `{name}` (or `{}`) placeholder in `template` with the next
 * entry of `args`, in order of appearance.
 */
function fill(template: string, args: readonly TranslateArg[]): string {
  let idx = 0;
  return template.replace(/\{[a-zA-Z0-9_]*\}/g, () =>
    idx < args.length ? String(args[idx++]) : ''
  );
}

export function translate(
  lang: Lang,
  key: string,
  args: readonly TranslateArg[] = []
): string {
  const template = CATALOGS[lang][key] ?? EN[key] ?? key;
  return fill(template, args);
}

/** First-run guess from the browser language. */
export function detectLang(): Lang {
  const nav = typeof navigator !== 'undefined' ? navigator.language : '';
  return nav.toLowerCase().startsWith('es') ? 'es' : 'en';
}

/** True when a backend message already carries the "next launch" suffix. */
export function hasLaunchHint(message: string): boolean {
  return /launches|inicie de nuevo/i.test(message);
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

interface I18nValue {
  lang: Lang;
  t: (key: string, args?: readonly TranslateArg[]) => string;
  /** Reconcile with the saved setting; returns the language now active. */
  applySaved: (saved?: string) => Lang;
  setLang: (lang: Lang) => void;
}

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(detectLang);

  const t = useCallback(
    (key: string, args: readonly TranslateArg[] = []) =>
      translate(lang, key, args),
    [lang]
  );

  const applySaved = useCallback((saved?: string): Lang => {
    const next: Lang =
      saved === 'es' || saved === 'en' ? saved : detectLang();
    setLangState(next);
    return next;
  }, []);

  const setLang = useCallback((next: Lang) => setLangState(next), []);

  const value = useMemo(
    () => ({ lang, t, applySaved, setLang }),
    [lang, t, applySaved, setLang]
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error('useI18n must be used within an <I18nProvider>');
  }
  return ctx;
}
