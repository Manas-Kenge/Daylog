import { useId } from "react";

interface SparklineProps {
  values: number[];
  color?: string;
  width?: number;
  height?: number;
  area?: boolean;
  className?: string;
  strokeWidth?: number;
}

export function Sparkline({
  values,
  color = "var(--chart-1)",
  width = 56,
  height = 14,
  area = false,
  className,
  strokeWidth = 1.25,
}: SparklineProps) {
  const gradId = useId();
  if (values.length < 2) return null;

  const max = Math.max(1, ...values);
  const min = Math.min(...values);
  const range = max - min || 1;
  const padding = area ? 3 : 1;
  const stepX = width / (values.length - 1);

  const pts = values.map((v, i) => {
    const x = i * stepX;
    const y = height - ((v - min) / range) * (height - 2 * padding) - padding;
    return [x, y];
  });
  const path = pts
    .map((p, i) => (i === 0 ? `M${p[0].toFixed(2)},${p[1].toFixed(2)}` : `L${p[0].toFixed(2)},${p[1].toFixed(2)}`))
    .join(" ");
  const areaPath = `${path} L${width},${height} L0,${height} Z`;

  return (
    <svg
      className={className}
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      width={className ? undefined : width}
      height={className ? undefined : height}
    >
      {area && (
        <defs>
          <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={color} stopOpacity="0.32" />
            <stop offset="100%" stopColor={color} stopOpacity="0" />
          </linearGradient>
        </defs>
      )}
      {area && <path d={areaPath} fill={`url(#${gradId})`} />}
      <path
        d={path}
        fill="none"
        stroke={color}
        strokeWidth={strokeWidth}
        strokeLinejoin="round"
        strokeLinecap="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
