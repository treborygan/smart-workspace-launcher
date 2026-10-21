import type { SVGProps } from "react";

const paths: Record<string, string> = {
  grid: "M3 3h7v7H3zM14 3h7v7h-7zM14 14h7v7h-7zM3 14h7v7H3z",
  search: "m20 20-4.4-4.4M18 10.5a7.5 7.5 0 1 1-15 0 7.5 7.5 0 0 1 15 0Z",
  plus: "M12 5v14M5 12h14",
  arrow: "M7 17 17 7M7 7h10v10",
  folder: "M3 6.5A1.5 1.5 0 0 1 4.5 5H10l2 2h7.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5Z",
  clock: "M12 8v4l2.5 1.5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z",
  chart: "M4 19V5m0 14h17M8 15l3-4 3 2 5-7",
  settings: "M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4ZM19.4 13.5a7.6 7.6 0 0 0 0-3l1.5-1.2-1.5-2.6-1.9.6a7.4 7.4 0 0 0-2.6-1.5L14.6 4h-3.1l-.3 1.8a7.4 7.4 0 0 0-2.6 1.5l-1.9-.6-1.5 2.6 1.5 1.2a7.6 7.6 0 0 0 0 3l-1.5 1.2 1.5 2.6 1.9-.6a7.4 7.4 0 0 0 2.6 1.5l.3 1.8h3.1l.3-1.8a7.4 7.4 0 0 0 2.6-1.5l1.9.6 1.5-2.6Z",
  star: "m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9Z",
  more: "M5 12h.01M12 12h.01M19 12h.01",
  close: "m18 6-12 12M6 6l12 12",
  back: "m15 18-6-6 6-6",
  chevron: "m9 18 6-6-6-6",
  moon: "M20.9 13A8.5 8.5 0 0 1 11 3.1 8.5 8.5 0 1 0 20.9 13Z",
};

export function Icon({ name, size = 18, ...props }: SVGProps<SVGSVGElement> & { name: keyof typeof paths; size?: number }) {
  const fill = name === "star" ? "currentColor" : "none";
  return <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill={fill} stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" {...props}><path d={paths[name]} /></svg>;
}
