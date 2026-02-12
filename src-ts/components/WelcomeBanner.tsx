import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Box, Text, useStdout } from 'ink';
import Gradient from 'ink-gradient';
import { useTheme } from '../theme/index.js';
import { useRustBridge } from '../hooks/useRustBridge.js';
import { Message } from '../types/index.js';
import { BANNER_BEGONIA } from '../theme/themes.js';

export const WelcomeBanner: React.FC = () => {
  const { t } = useTranslation();
  const { theme } = useTheme();
  const { getAppConfig } = useRustBridge();
  const { stdout } = useStdout();
  const terminalWidth = stdout?.columns || 80;
  
  const [data] = useState<{ tips: string[] }>(() => {
    try {
      const config = getAppConfig();
      return {
        tips: config.welcome?.tips || []
      };
    } catch (error) {
      return { tips: [] };
    }
  });

  // Use BANNER_BEGONIA as the banner text
  const bannerLines = BANNER_BEGONIA.trim().split('\n');
  
  // Use config tips if available, otherwise use i18n tips
  const tips = data.tips.length > 0 
    ? data.tips 
    : (t('welcome.tips', { returnObjects: true }) as string[]);

  // Determine gradient props
  const gradientProps = Array.isArray(theme.colors.bannerGradient)
    ? { colors: theme.colors.bannerGradient }
    : { name: theme.colors.bannerGradient as any };

  // Generate full-width horizontal line
  const horizontalLine = '─'.repeat(terminalWidth);

  return (
    <Box flexDirection="column" padding={1} paddingBottom={1}>
        <Box flexDirection="column" alignItems="center" width={terminalWidth}>
          {bannerLines.map((line, index) => (
            line === '' ? (
              <Text key={index}> </Text>
            ) : (
              <Gradient key={index} {...gradientProps}>
                <Text>{line}</Text>
              </Gradient>
            )
          ))}
        </Box>
        {tips.length > 0 && (
          <Box flexDirection="column" marginTop={1} width={terminalWidth} alignItems="center">
            <Text color={theme.colors.dimText}>{horizontalLine}</Text>
            <Box flexDirection="column" alignItems="flex-start">
              {tips.map((tip, index) => (
                <Text key={index} color={theme.colors.dimText}>
                  {"● "}{tip}
                </Text>
              ))}
            </Box>
            <Text color={theme.colors.dimText}>{horizontalLine}</Text>
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
