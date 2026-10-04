import { useRef, useState, type PointerEvent } from "react";
import type { HudBlock } from "../types";

interface BlockOrderListProps {
  order: HudBlock[];
  labels: Record<HudBlock, string>;
  /** Disabled blocks are dimmed but can still be moved. */
  isEnabled: (block: HudBlock) => boolean;
  accentColor: string;
  onChange: (order: HudBlock[]) => void;
}

/**
 * Drag-to-reorder list built on pointer events. Native HTML5 drag and drop is avoided because
 * Tauri's file-drop handling on Windows swallows those events.
 */
export default function BlockOrderList({ order, labels, isEnabled, accentColor, onChange }: BlockOrderListProps) {
  const [dragged, setDragged] = useState<HudBlock | null>(null);
  const [preview, setPreview] = useState<HudBlock[] | null>(null);
  const rowRefs = useRef(new Map<HudBlock, HTMLLIElement>());
  const items = preview ?? order;

  const startDrag = (event: PointerEvent<HTMLElement>, block: HudBlock) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragged(block);
    setPreview(order);
  };

  const moveDrag = (event: PointerEvent<HTMLElement>) => {
    if (!dragged || !preview) return;
    const others = preview.filter((block) => block !== dragged);
    const target = others.filter((block) => {
      const rect = rowRefs.current.get(block)?.getBoundingClientRect();
      return rect != null && rect.top + rect.height / 2 < event.clientY;
    }).length;
    const next = [...others.slice(0, target), dragged, ...others.slice(target)];
    if (next.some((block, i) => block !== preview[i])) setPreview(next);
  };

  const endDrag = () => {
    if (preview && preview.some((block, i) => block !== order[i])) onChange(preview);
    setDragged(null);
    setPreview(null);
  };

  return (
    <ul className="flex flex-col gap-1">
      {items.map((block, index) => (
        <li
          key={block}
          ref={(el) => {
            if (el) rowRefs.current.set(block, el);
            else rowRefs.current.delete(block);
          }}
          className={`flex items-center gap-2 rounded-md border px-2 py-1 text-sm transition-colors ${
            isEnabled(block) ? "text-white/80" : "text-white/30"
          }`}
          style={{
            borderColor: block === dragged ? accentColor : "rgba(255,255,255,0.08)",
            background: block === dragged ? "rgba(255,255,255,0.06)" : undefined,
          }}
        >
          <span
            className="cursor-grab touch-none select-none px-1 text-white/40 active:cursor-grabbing"
            onPointerDown={(e) => startDrag(e, block)}
            onPointerMove={moveDrag}
            onPointerUp={endDrag}
            onPointerCancel={endDrag}
            aria-label={`Drag ${labels[block]}`}
          >
            ⋮⋮
          </span>
          <span className="w-4 text-xs tabular-nums text-white/30">{index + 1}</span>
          {labels[block]}
        </li>
      ))}
    </ul>
  );
}
