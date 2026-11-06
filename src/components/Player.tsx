import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Pause, Play } from "lucide-react";
import { fmtTime } from "../lib/format";

interface Props {
  path: string;
  name: string;
  meta: string;
  peaks: number[] | null;
  onSwap: () => void;
}

/** Odtwarzacz + waveform (port `drawWave` z prototypu); klik w przebieg = przewijanie. */
export function Player({ path, name, meta, peaks, onSwap }: Props) {
  const audio = useRef<HTMLAudioElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const raf = useRef(0);
  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const draw = useCallback(() => {
    const cv = canvas.current;
    const au = audio.current;
    if (!cv) return;
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth;
    const h = cv.clientHeight;
    if (!w) return;
    cv.width = w * dpr;
    cv.height = h * dpr;
    const c = cv.getContext("2d");
    if (!c) return;
    c.scale(dpr, dpr);
    const cs = getComputedStyle(document.documentElement);
    const on = cs.getPropertyValue("--wave").trim();
    const off = cs.getPropertyValue("--wave-dim").trim();
    const prog = au && au.duration ? au.currentTime / au.duration : 0;
    const bw = 3;
    const gap = 2;
    const n = Math.floor(w / (bw + gap));
    for (let i = 0; i < n; i++) {
      let v: number;
      if (peaks && peaks.length) {
        const a = Math.floor((i / n) * peaks.length);
        const b = Math.max(a + 1, Math.floor(((i + 1) / n) * peaks.length));
        v = 0;
        for (let k = a; k < b; k++) v = Math.max(v, peaks[k]);
      } else {
        v = 0.12 + 0.08 * Math.sin(i * 0.7);
      }
      const bh = Math.max(2, v * (h - 8));
      c.fillStyle = i / n < prog ? on : off;
      c.beginPath();
      c.roundRect(i * (bw + gap), (h - bh) / 2, bw, bh, 1.5);
      c.fill();
    }
  }, [peaks]);

  useEffect(() => {
    draw();
    const ro = new ResizeObserver(draw);
    if (canvas.current) ro.observe(canvas.current);
    const mq = matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", draw);
    return () => {
      ro.disconnect();
      mq.removeEventListener("change", draw);
    };
  }, [draw]);

  useEffect(() => {
    if (!playing) {
      cancelAnimationFrame(raf.current);
      draw();
      return;
    }
    const loop = () => {
      draw();
      raf.current = requestAnimationFrame(loop);
    };
    loop();
    return () => cancelAnimationFrame(raf.current);
  }, [playing, draw]);

  const toggle = () => {
    const au = audio.current;
    if (!au) return;
    if (au.paused)
      au.play().catch(() =>
        setError("Ten format nie odtworzy się tutaj — Gemini i tak go przeanalizuje."),
      );
    else au.pause();
  };

  const seek = (e: React.MouseEvent<HTMLDivElement>) => {
    const au = audio.current;
    if (!au || !au.duration) return;
    const r = e.currentTarget.getBoundingClientRect();
    au.currentTime = ((e.clientX - r.left) / r.width) * au.duration;
    draw();
  };

  return (
    <div className="deck">
      <div className="deck-top">
        <button className="play" onClick={toggle} aria-label={playing ? "Pauza" : "Odtwórz"}>
          {playing ? (
            <Pause size={18} fill="currentColor" />
          ) : (
            <Play size={18} fill="currentColor" />
          )}
        </button>
        <div className="deck-meta">
          <b>{name}</b>
          <span className="file-meta">
            {fmtTime(duration)} · {meta}
          </span>
        </div>
        <button className="swap" onClick={onSwap}>
          Zmień plik
        </button>
      </div>
      <div className="wave" onClick={seek} title="Kliknij, żeby przewinąć">
        <canvas ref={canvas} aria-label="Przebieg utworu" />
      </div>
      {error && <span className="hint">{error}</span>}
      <audio
        ref={audio}
        src={convertFileSrc(path)}
        preload="metadata"
        onLoadedMetadata={(e) => setDuration(e.currentTarget.duration)}
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
        hidden
      />
    </div>
  );
}
