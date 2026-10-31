import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Section } from "../app/store";

/** Exit duration — must match the veil-section-exit keyframe / reduced-motion snap. */
const EXIT_MS = 150;

interface SectionTransitionProps {
  section: Section;
  /** Renders the content for a given section. Called with the section being displayed. */
  children: (section: Section) => ReactNode;
}

/**
 * Crossfades between top-level sections: the outgoing section plays an exit animation
 * before the incoming one mounts and plays its entrance. View-only, compositor-only
 * (opacity + transform). Honors prefers-reduced-motion via the CSS snap in app.css.
 */
export function SectionTransition({ section, children }: SectionTransitionProps): ReactNode {
  const [displayed, setDisplayed] = useState<Section>(section);
  const [state, setState] = useState<"entering" | "exiting">("entering");
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    if (section === displayed) return;
    setState("exiting");
    if (timerRef.current !== undefined) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      setDisplayed(section);
      setState("entering");
      timerRef.current = undefined;
    }, EXIT_MS);
    return () => {
      if (timerRef.current !== undefined) clearTimeout(timerRef.current);
    };
  }, [section, displayed]);

  return (
    <div key={displayed} className="veil-section-transition" data-state={state}>
      {children(displayed)}
    </div>
  );
}
