import { useEffect } from "react";
import { AudioLines, Clock, FileText, Settings2, Users, type LucideIcon } from "lucide-react";
import { Brand } from "./components/Brand";
import { useApp, type Tab } from "./lib/store";
import { Run } from "./screens/Run";
import { Settings } from "./screens/Settings";
import { Wizard } from "./screens/Wizard";
import "./styles/app.css";

const TABS: { id: Tab; label: string; icon: LucideIcon }[] = [
  { id: "run", label: "Analiza i pitch", icon: AudioLines },
  { id: "artists", label: "Artyści", icon: Users },
  { id: "history", label: "Historia", icon: Clock },
  { id: "rules", label: "Zasady", icon: FileText },
  { id: "settings", label: "Ustawienia", icon: Settings2 },
];

function Soon({ title }: { title: string }) {
  return (
    <section className="view">
      <div className="view-head">
        <h1>{title}</h1>
        <p className="lede">Ten ekran powstanie w kolejnym etapie.</p>
      </div>
    </section>
  );
}

export default function App() {
  const { ready, tab, setTab, settings, hasGemini, toast, load } = useApp();

  useEffect(() => {
    load();
  }, [load]);

  if (!ready) return null;
  // Bez klucza i bez ukończonego kreatora reszta aplikacji jest zablokowana.
  if (!hasGemini || settings.onboarded !== "1")
    return (
      <>
        <Wizard />
        {toast && <div className="toast">{toast}</div>}
      </>
    );

  return (
    <>
      <header className="top">
        <div className="top-in">
          <Brand />
          <nav className="tabs" role="tablist" aria-label="Sekcje">
            {TABS.map(({ id, label, icon: Icon }) => (
              <button key={id} role="tab" aria-selected={tab === id} onClick={() => setTab(id)}>
                <Icon className="i" aria-hidden="true" size={16} />
                <span className="lab">{label}</span>
              </button>
            ))}
          </nav>
          <div className="status-pill">
            <span className="led" />
            {settings.gemini_model || "wybierz model"}
          </div>
        </div>
      </header>
      <main className="wrap">
        <div hidden={tab !== "run"}>
          <Run active={tab === "run"} />
        </div>
        {tab === "settings" && <Settings />}
        {tab !== "run" && tab !== "settings" && (
          <Soon title={TABS.find((t) => t.id === tab)!.label} />
        )}
      </main>
      {toast && <div className="toast">{toast}</div>}
    </>
  );
}
