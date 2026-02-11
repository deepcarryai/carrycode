import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Box, Text, useInput } from 'ink';
import { useTheme } from '../theme/index.js';

interface ToolConfirmMenuProps {
  request: {
    toolName?: string;
    arguments?: string;
    kind?: string;
    keyPath?: string;
  };
  onConfirm: (decision: string) => void;
}

export function ToolConfirmMenu({ request, onConfirm }: ToolConfirmMenuProps) {
  const { t } = useTranslation();
  const { theme } = useTheme();
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const options = useMemo(
    () => [
      { id: '1', label: t('confirm.yes_execute') },
      { id: '2', label: t('confirm.yes_session') },
      { id: '3', label: t('confirm.no_differently') },
    ],
    [t],
  );

  useEffect(() => {
    setSelectedIndex(0);
    setIsSubmitting(false);
  }, [request.toolName, request.arguments, request.kind, request.keyPath]);

  const submit = (id: string) => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    setSelectedIndex(0);
    onConfirm(id);
  };

  useInput(
    (input, key) => {
      if (isSubmitting) return;
      if (key.upArrow) {
        setSelectedIndex((prev) => Math.max(0, prev - 1));
        return;
      }
      if (key.downArrow) {
        setSelectedIndex((prev) => Math.min(options.length - 1, prev + 1));
        return;
      }
      if (key.return) {
        submit(options[selectedIndex]?.id ?? '1');
        return;
      }
      if (input === '1' || input === '2' || input === '3') {
        submit(input);
      }
    },
    { isActive: !isSubmitting },
  );

  return (
    <Box flexDirection="column" borderStyle="round" borderColor={theme.colors.warning} paddingX={1}>
      <Text wrap="truncate">
        {t('confirm.tool')} <Text color={theme.colors.primary}>{String(request.toolName ?? '')}</Text>
      </Text>
      {request.keyPath && request.keyPath !== '*' && (
        <Text wrap="truncate">
          {t('confirm.target')} <Text color={theme.colors.secondary}>{request.keyPath}</Text>
        </Text>
      )}
      {options.map((option, index) => (
        <Text
          key={option.id}
          color={index === selectedIndex ? theme.colors.success : theme.colors.text}
          wrap="truncate"
        >
          {index === selectedIndex ? '> ' : '  '}
          {option.id}. {option.label}
        </Text>
      ))}
    </Box>
  );
}

