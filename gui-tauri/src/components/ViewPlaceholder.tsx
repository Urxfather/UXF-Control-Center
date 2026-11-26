import type { NavSection } from "../types/navigation";

interface ViewPlaceholderProps {
  section: NavSection;
}

export function ViewPlaceholder({ section }: ViewPlaceholderProps) {
  return (
    <section
      aria-labelledby="section-heading"
      className="flex h-full flex-col"
    >
      <div className="mb-6">
        <h2
          id="section-heading"
          className="text-lg font-medium tracking-wide text-slate-100"
        >
          {section.label}
        </h2>
        <div className="mt-2.5 flex items-center">
          <div className="h-[2px] w-20 bg-[#00d8ff]" />
          <div className="h-[1px] flex-1 bg-[#262b36]" />
        </div>
      </div>

      <div className="flex flex-1 items-center justify-center">
        <div className="w-full max-w-lg rounded border border-[#222834] bg-[#0d1016] p-6 shadow-md">
          <div className="mb-3 flex items-center justify-between gap-3 border-b border-[#1f2630] pb-3">
            <span className="text-[11px] font-semibold tracking-[0.16em] text-[#00d8ff] uppercase">
              {section.label}
            </span>
            <span className="rounded border border-[#2a3342] bg-[#070a0f] px-2.5 py-0.5 font-mono text-[10px] tracking-[0.08em] text-[#9be000]/90 uppercase">
              Coming Soon
            </span>
          </div>

          <p className="text-base font-semibold text-white">
            {section.placeholderHeading}
          </p>
          <p className="mt-2 text-sm leading-relaxed text-[#7c8594]">
            {section.placeholderNote}
          </p>
        </div>
      </div>
    </section>
  );
}
