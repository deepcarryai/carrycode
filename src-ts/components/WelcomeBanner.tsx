import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Box, Text } from 'ink';
import BigText from 'ink-big-text';
import Gradient from 'ink-gradient';
import { useTheme } from '../theme/index.js';
import { useRustBridge } from '../hooks/useRustBridge.js';
import { Message } from '../types/index.js';
import tinyFont from 'cfonts/fonts/tiny.json';

export const WelcomeBanner: React.FC = () => {
  const { t } = useTranslation();
  const { theme } = useTheme();
  const { getAppConfig } = useRustBridge();
  
  const [data] = useState<{ lines: string[] }>(() => {
    try {
      const config = getAppConfig();
      return {
        lines: config.welcome?.banner || [],
      };
    } catch (error) {
      return { lines: [] };
    }
  });

  const { lines } = data;
  const [bundledFont] = useState(() => tinyFont);
  
  const tips = t('welcome.tips', { returnObjects: true }) as string[];

  if (lines.length === 0) {
    return null;
  }

  // Determine gradient props
  const gradientProps = Array.isArray(theme.colors.bannerGradient)
    ? { colors: theme.colors.bannerGradient }
    : { name: theme.colors.bannerGradient as any };

  return (
    <Box flexDirection="column" padding={1} paddingBottom={1}>
      <Gradient {...gradientProps}>
        <BigText
          text={lines.join('\n')}
          font="shade"
          // font={bundledFont ? "tiny" : "tiny"}
          lineHeight={0.5}
        />
      </Gradient>
      {tips.length > 0 && (
        <Box flexDirection="column" marginTop={0}>
          {tips.map((tip, index) => (
            <Text key={index} color={theme.colors.dimText}>
              {"● "}{tip}
            </Text>
          ))}
        </Box>
      )}
    </Box>
  );
};

export function createWelcomeMessage(): Message {
    return {
        question: '',
        segments: [
            {
                stage: '__ANSWERING__',
                title: 'Welcome Banner',
                content: '',
                tools: [],
                isBanner: true,
            }
        ],
        startTime: Date.now(),
        duration: 0
    };
}
