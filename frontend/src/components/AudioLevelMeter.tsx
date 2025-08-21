'use client';

import React from 'react';

interface AudioLevelMeterProps {
  level: number; // 0 to 1
  peak: number; // 0 to 1
  className?: string;
}

export const AudioLevelMeter: React.FC<AudioLevelMeterProps> = ({ 
  level, 
  peak,
  className = '' 
}) => {
  // Convert to percentage for display
  const levelPercent = Math.min(100, level * 100);
  const peakPercent = Math.min(100, peak * 100);
  
  // Determine color based on level
  const getColor = (percent: number) => {
    if (percent < 50) return 'bg-green-500';
    if (percent < 75) return 'bg-yellow-500';
    return 'bg-red-500';
  };

  return (
    <div className={`flex items-center gap-1 ${className}`}>
      {/* Level bars */}
      <div className="flex gap-0.5 h-8 items-end">
        {[...Array(10)].map((_, i) => {
          const barThreshold = (i + 1) * 10;
          const isActive = levelPercent >= barThreshold;
          const isPeak = peakPercent >= barThreshold && peakPercent < barThreshold + 10;
          
          return (
            <div
              key={i}
              data-testid={i === 7 ? 'audio-level-meter' : undefined}
              className={`w-1.5 transition-all duration-75 rounded-sm ${
                isActive 
                  ? getColor(barThreshold)
                  : isPeak
                  ? 'bg-gray-500 opacity-50'
                  : 'bg-gray-700'
              }`}
              style={{
                height: `${20 + (i * 8)}%`,
                opacity: isActive ? 1 : 0.3
              }}
            />
          );
        })}
      </div>
    </div>
  );
};

export default AudioLevelMeter;