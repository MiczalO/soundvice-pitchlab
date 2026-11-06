const BARS = [6, 14, 9, 16, 5];

export function Brand() {
  return (
    <span className="brand" aria-label="Soundvice pitchLAB">
      <span className="brand-mark" aria-hidden="true">
        <span>
          {BARS.map((h, i) => (
            <i key={i} style={{ height: h }} />
          ))}
        </span>
      </span>
      <span className="brand-text">
        <b>Soundvice</b>
        <em>
          pitch<strong>LAB</strong>
        </em>
      </span>
    </span>
  );
}
