import { useEffect, useState, useSyncExternalStore } from "react";
import {
  AudioLines,
  Clock,
  FileText,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Settings2,
  Sun,
  Users,
  type LucideIcon,
} from "lucide-react";
import { Brand } from "./components/Brand";
import { useApp, type Tab, type Theme } from "./lib/store";
import { Artists } from "./screens/Artists";
import { History } from "./screens/History";
import { Rules } from "./screens/Rules";
import { Run } from "./screens/Run";
import { Settings } from "./screens/Settings";
import { Wizard } from "./screens/Wizard";
import "./styles/app.css";

const NAV: { id: Tab; label: string; icon: LucideIcon }[] = [
  { id: "run", label: "Analiza i pitch", icon: AudioLines },
  { id: "artists", label: "Artyści", icon: Users },
  { id: "history", label: "Historia", icon: Clock },
  { id: "rules", label: "Zasady", icon: FileText },
  { id: "settings", label: "Ustawienia", icon: Settings2 },
];

const THEMES: { id: Theme; label: string; icon: LucideIcon }[] = [
  { id: "system", label: "systemowy", icon: Monitor },
  { id: "light", label: "jasny", icon: Sun },
  { id: "dark", label: "ciemny", icon: Moon },
];

/** Preferencja paska bocznego: `auto` = zwija się w wąskim oknie, resztę ustala użytkownik. */
type Side = "auto" | "expanded" | "collapsed";
const SIDE_KEY = "pl_side";

function readSide(): Side {
  try {
    const v = localStorage.getItem(SIDE_KEY);
    return v === "expanded" || v === "collapsed" ? v : "auto";
  } catch {
    return "auto";
  }
}

function useNarrow(): boolean {
  return useSyncExternalStore(
    (cb) => {
      const mq = matchMedia("(max-width: 1100px)");
      mq.addEventListener("change", cb);
      return () => mq.removeEventListener("change", cb);
    },
    () => matchMedia("(max-width: 1100px)").matches,
  );
}

export default function App() {
  const { ready, tab, setTab, settings, hasGemini, toast, load, saveSetting } = useApp();
  const [side, setSide] = useState<Side>(readSide);
  const narrow = useNarrow();
  const collapsed = side === "collapsed" || (side === "auto" && narrow);

  useEffect(() => {
    load();
  }, [load]);

  const toggleSide = () => {
    const next: Side = collapsed ? "expanded" : "collapsed";
    setSide(next);
    try {
      localStorage.setItem(SIDE_KEY, next);
    } catch {
      /* preferencja tylko w tej sesji */
    }
  };

  const goto = (id: Tab) => {
    setTab(id);
    window.scrollTo({ top: 0 });
  };

  if (!ready) return null;
  // Bez klucza i bez ukończonego kreatora reszta aplikacji jest zablokowana.
  if (!hasGemini || settings.onboarded !== "1")
    return (
      <>
        <Wizard />
        {toast && (
          <div className="toast" role="status">
            {toast}
          </div>
        )}
      </>
    );

  const theme = THEMES.find((t) => t.id === (settings.theme ?? "system")) ?? THEMES[0];
  const nextTheme = THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length];
  const ThemeIcon = theme.icon;

  return (
    <div className="app" data-collapsed={collapsed} data-user-expanded={side === "expanded"}>
      <a className="skip" href="#main">
        Przejdź do treści
      </a>
      <aside className="sidebar" aria-label="Pasek boczny">
        <div className="side-head">
          <Brand />
          <button
            className="icon-btn"
            onClick={toggleSide}
            aria-label={collapsed ? "Rozwiń pasek boczny" : "Zwiń pasek boczny"}
            aria-expanded={!collapsed}
          >
            {collapsed ? <PanelLeftOpen size={20} /> : <PanelLeftClose size={20} />}
          </button>
        </div>
        <nav className="side-nav" aria-label="Sekcje">
          {NAV.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className="nav-item"
              aria-current={tab === id ? "page" : undefined}
              aria-label={label}
              data-tip={label}
              onClick={() => goto(id)}
            >
              <Icon aria-hidden="true" />
              <span className="lab">{label}</span>
            </button>
          ))}
        </nav>
        <div className="side-foot">
          <div className="status-pill" data-tip={settings.gemini_model || "wybierz model"}>
            <span className="led" aria-hidden="true" />
            <span className="lab">{settings.gemini_model || "wybierz model"}</span>
          </div>
          <button
            className="nav-item"
            onClick={() => saveSetting("theme", nextTheme.id)}
            aria-label={`Motyw: ${theme.label}. Zmień na: ${nextTheme.label}`}
            data-tip={`Motyw: ${theme.label}`}
          >
            <ThemeIcon aria-hidden="true" />
            <span className="lab">Motyw: {theme.label}</span>
          </button>
        </div>
      </aside>
      <main className="main" id="main" tabIndex={-1}>
        <div className="wrap">
          <div hidden={tab !== "run"}>
            <Run active={tab === "run"} />
          </div>
          <div hidden={tab !== "artists"}>
            <Artists active={tab === "artists"} />
          </div>
          {tab === "history" && <History />}
          {tab === "rules" && <Rules />}
          {tab === "settings" && <Settings />}
        </div>
      </main>
      {toast && (
        <div className="toast" role="status">
          {toast}
        </div>
      )}
    </div>
  );
}
