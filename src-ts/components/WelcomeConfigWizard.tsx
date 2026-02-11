import React, { useMemo, useState, useEffect } from 'react';
import { Box, Text, useInput, useStdout } from 'ink';
import SelectInput from 'ink-select-input';
import { Stepper, Step } from 'ink-stepper';
import type { ProgressContext, StepContext } from 'ink-stepper';
import { useTranslation } from 'react-i18next';
import stringWidth from 'string-width';
import { useTheme } from '../theme/index.js';
import { useRustBridge } from '../hooks/useRustBridge.js';
import i18n from '../i18n/index.js';
import { logger } from '../utils/logger.js';
import { backspaceByGraphemeApprox, padLeftToWidth, padRightToWidth, truncateToWidth, getCachedStringWidth } from '../utils/textUtils.js';
import type { ProviderPreset } from '../hooks/useRustBridge.js';

type LanguageOption = 'en' | 'zh-CN';
type AgentModeOption = 'plan' | 'build';

export interface WelcomeConfigWizardProps {
  sessionId: string;
  onFinished: () => void;
  onCancelled: () => void;
}

type FieldKey = 'providerName' | 'baseUrl' | 'apiKey' | 'modelName' | 'next';

function maskSecret(value: string): string {
  if (!value) return '';
  if (value.length <= 4) return '*'.repeat(value.length);
  return '*'.repeat(value.length - 4) + value.slice(-4);
}

function isLikelyUrl(value: string): boolean {
  const v = value.trim();
  return v.startsWith('http://') || v.startsWith('https://');
}

function WizardTitleArea(props: { title: string; subtitle?: string; colors: { primary: string } }) {
  const { title, subtitle, colors } = props;
  return (
    <Box flexDirection="column" width="100%">
      <Text color={colors.primary} bold>
        {title}
      </Text>
      {subtitle ? <Text dimColor>{subtitle}</Text> : null}
    </Box>
  );
}

function WizardProgressBar(props: {
  progress: ProgressContext;
  width: number;
  colors: { success: string; text: string };
  labelForStepName: (name: string) => string;
}) {
  const { progress, width, colors, labelForStepName } = props;
  const innerWidth = Math.max(20, Math.floor(width));
  const steps = progress.steps ?? [];
  const n = steps.length;

  if (n <= 0) return null;

  const posFor = (i: number) => {
    if (n <= 1) return 0;
    return Math.round((i * (innerWidth - 1)) / (n - 1));
  };

  const bar = new Array<string>(innerWidth).fill('─');
  for (let i = 0; i < n; i++) {
    const p = posFor(i);
    const s = steps[i]!;
    bar[p] = s.current || s.completed ? '●' : '○';
  }

  const markerColor = new Array<string>(innerWidth).fill(colors.text);
  for (let i = 0; i < n; i++) {
    const p = posFor(i);
    const s = steps[i]!;
    if (s.completed) markerColor[p] = colors.success;
  }

  const labels = new Array<string>(innerWidth).fill(' ');
  const segmentWidth = Math.max(6, Math.floor(innerWidth / n));
  const truncateToWidth = (text: string, maxWidth: number) => {
    if (maxWidth <= 0) return '';
    let out = '';
    let w = 0;
    for (const ch of Array.from(text)) {
      const cw = stringWidth(ch);
      if (cw <= 0) continue;
      if (w + cw > maxWidth) break;
      out += ch;
      w += cw;
    }
    return out;
  };

  const trimLabel = (label: string) => {
    if (stringWidth(label) <= segmentWidth) return label;
    if (segmentWidth <= 1) return truncateToWidth(label, 1);
    return truncateToWidth(label, segmentWidth - 1) + '…';
  };

  for (let i = 0; i < n; i++) {
    const p = posFor(i);
    const name = trimLabel(labelForStepName(String(steps[i]!.name ?? '')));
    const nameWidth = stringWidth(name);
    const start = Math.max(0, Math.min(innerWidth - nameWidth, p - Math.floor(nameWidth / 2)));
    let col = start;
    for (const ch of Array.from(name)) {
      const cw = stringWidth(ch);
      if (cw <= 0) continue;
      if (col >= innerWidth) break;
      if (cw === 1) {
        if (labels[col] === ' ') labels[col] = ch;
        col += 1;
        continue;
      }
      if (cw === 2) {
        if (col + 1 >= innerWidth) break;
        if (labels[col] === ' ' && labels[col + 1] === ' ') {
          labels[col] = ch;
          labels[col + 1] = '';
        }
        col += 2;
      } else {
        col += 1;
      }
    }
  }

  let labelsLine = labels.join('');
  const labelsPad = innerWidth - stringWidth(labelsLine);
  if (labelsPad > 0) labelsLine += ' '.repeat(labelsPad);

  const renderBar = () => {
    const nodes: React.ReactNode[] = [];
    let runStart = 0;
    let runColor: string = markerColor[0]!;

    const pushRun = (start: number, end: number, color: string) => {
      const text = bar.slice(start, end).join('');
      if (!text) return;
      nodes.push(
        <Text key={`${start}-${end}`} color={color ?? undefined}>
          {text}
        </Text>,
      );
    };

    for (let i = 1; i <= innerWidth; i++) {
      const c = i < innerWidth ? markerColor[i]! : markerColor[innerWidth - 1]!;
      if (c !== runColor) {
        pushRun(runStart, i, runColor);
        runStart = i;
        runColor = c;
      }
    }
    pushRun(runStart, innerWidth, runColor);
    return (
      <Box flexDirection="row" flexWrap="nowrap" width="100%">
        {nodes}
      </Box>
    );
  };

  return (
    <Box flexDirection="column" width="100%">
      {renderBar()}
      <Text dimColor>{labelsLine}</Text>
    </Box>
  );
}

function LanguageStepContent(props: {
  ctx: StepContext;
  languageItems: Array<{ label: string; value: LanguageOption }>;
  onSelect: (next: LanguageOption) => void;
  onTitle: (title: string, subtitle?: string) => void;
  onHint: (hint: string) => void;
}) {
  const { t } = useTranslation();
  const { ctx, languageItems, onSelect, onTitle, onHint } = props;
  useEffect(() => {
    onTitle(t('welcome_wizard.titles.language'));
    onHint(t('welcome_wizard.hints.list'));
  }, [onHint, onTitle, t]);
  useInput((_, key) => {
    if (key.escape) ctx.cancel();
  });

  return (
    <Box flexDirection="column" marginTop={1}>
      <SelectInput
        items={languageItems}
        onSelect={(item) => {
          onSelect(item.value as LanguageOption);
          ctx.goNext();
        }}
      />
    </Box>
  );
}

function ThemeStepContent(props: {
  ctx: StepContext;
  themeItems: Array<{ label: string; value: string }>;
  onSelect: (next: string) => void;
  onTitle: (title: string, subtitle?: string) => void;
  onHint: (hint: string) => void;
}) {
  const { t } = useTranslation();
  const { ctx, themeItems, onSelect, onTitle, onHint } = props;
  useEffect(() => {
    onTitle(t('welcome_wizard.titles.theme'));
    onHint(t('welcome_wizard.hints.list'));
  }, [onHint, onTitle, t]);
  useInput((_, key) => {
    if (key.escape) ctx.goBack();
  });

  return (
    <Box flexDirection="column" marginTop={1}>
      <SelectInput
        items={themeItems}
        onSelect={(item) => {
          onSelect(String(item.value));
          ctx.goNext();
        }}
      />
    </Box>
  );
}

function AgentStepContent(props: {
  ctx: StepContext;
  agentItems: Array<{ label: string; value: AgentModeOption }>;
  onSelect: (next: AgentModeOption) => void;
  onTitle: (title: string, subtitle?: string) => void;
  onHint: (hint: string) => void;
}) {
  const { t } = useTranslation();
  const { ctx, agentItems, onSelect, onTitle, onHint } = props;
  useEffect(() => {
    onTitle(t('welcome_wizard.titles.agent'));
    onHint(t('welcome_wizard.hints.list'));
  }, [onHint, onTitle, t]);
  useInput((_, key) => {
    if (key.escape) ctx.goBack();
  });

  return (
    <Box flexDirection="column" marginTop={1}>
      <SelectInput
        items={agentItems}
        onSelect={(item) => {
          onSelect(item.value as AgentModeOption);
          ctx.goNext();
        }}
      />
    </Box>
  );
}

function ProviderStepContent(props: {
  ctx: StepContext;
  themeColors: { success: string; text: string };
  inputWidth: number;
  providerBrandItems: Array<{ label: string; value: string }>;
  providerBrand: string;
  setProviderBrand: (v: string) => void;
  providerPhase: 'brand' | 'edit';
  setProviderPhase: (v: 'brand' | 'edit') => void;
  activeField: FieldKey;
  setActiveField: (v: FieldKey | ((prev: FieldKey) => FieldKey)) => void;
  providerName: string;
  setProviderName: (v: string | ((prev: string) => string)) => void;
  setProviderNameTouched: (v: boolean) => void;
  baseUrl: string;
  setBaseUrl: (v: string | ((prev: string) => string)) => void;
  setBaseUrlTouched: (v: boolean) => void;
  apiKey: string;
  setApiKey: (v: string | ((prev: string) => string)) => void;
  modelName: string;
  setModelName: (v: string | ((prev: string) => string)) => void;
  applyPreset: (brand: string) => void;
  onTitle: (title: string, subtitle?: string) => void;
  onHint: (hint: string) => void;
  onError: (message: string) => void;
}) {
  const { t } = useTranslation();
  const {
    ctx,
    themeColors,
    inputWidth,
    providerBrandItems,
    providerBrand,
    setProviderBrand,
    providerPhase,
    setProviderPhase,
    activeField,
    setActiveField,
    providerName,
    setProviderName,
    setProviderNameTouched,
    baseUrl,
    setBaseUrl,
    setBaseUrlTouched,
    apiKey,
    setApiKey,
    modelName,
    setModelName,
    applyPreset,
    onTitle,
    onHint,
    onError,
  } = props;

  useEffect(() => {
    if (providerPhase === 'brand') {
      onTitle(t('welcome_wizard.titles.provider_brand'));
    } else {
      onTitle(t('welcome_wizard.titles.provider_details'), providerBrand ? `(${providerBrand})` : undefined);
    }
    onHint(t('welcome_wizard.hints.provider'));
  }, [onHint, onTitle, providerBrand, providerPhase, t]);

  useInput((input, key) => {
    if (providerPhase === 'brand') {
      if (key.escape) ctx.goBack();
      return;
    }

    if (key.escape) {
      setProviderPhase('brand');
      return;
    }

    if (key.tab && !key.shift) {
      setActiveField((prev) => {
        if (prev === 'providerName') return 'baseUrl';
        if (prev === 'baseUrl') return 'apiKey';
        if (prev === 'apiKey') return 'modelName';
        if (prev === 'modelName') return 'next';
        return 'providerName';
      });
      return;
    }

    if (key.tab && key.shift) {
      setActiveField((prev) => {
        if (prev === 'providerName') return 'next';
        if (prev === 'baseUrl') return 'providerName';
        if (prev === 'apiKey') return 'baseUrl';
        if (prev === 'modelName') return 'apiKey';
        return 'modelName';
      });
      return;
    }

    if (key.return) {
      if (activeField === 'providerName') setActiveField('baseUrl');
      else if (activeField === 'baseUrl') setActiveField('apiKey');
      else if (activeField === 'apiKey') setActiveField('modelName');
      else if (activeField === 'modelName') setActiveField('next');
      else {
        const missing: Array<{ k: FieldKey; name: string }> = [];
  if (!providerName.trim()) missing.push({ k: 'providerName', name: 'provider_id' });
        if (!baseUrl.trim()) missing.push({ k: 'baseUrl', name: 'base_url' });
        if (!apiKey) missing.push({ k: 'apiKey', name: 'api_key' });
        if (!modelName.trim()) missing.push({ k: 'modelName', name: 'model_name' });

        if (missing.length > 0) {
          const sep = t('welcome_wizard.list_separator');
          onError(t('welcome_wizard.errors.fill_first', { fields: missing.map((m) => m.name).join(sep) }));
          setActiveField(missing[0]!.k);
          return;
        }

        onError('');
        ctx.goNext();
      }
      return;
    }

    const isBackspace =
      Boolean(key.backspace) || Boolean((key as any).delete) || input === '\u007f' || input === '\b';
    if (isBackspace) {
      if (activeField === 'providerName') {
        setProviderName((v) => backspaceByGraphemeApprox(v));
        setProviderNameTouched(true);
        onError('');
      } else if (activeField === 'baseUrl') {
        setBaseUrl((v) => backspaceByGraphemeApprox(v));
        setBaseUrlTouched(true);
        onError('');
      } else if (activeField === 'apiKey') {
        setApiKey((v) => backspaceByGraphemeApprox(v));
        onError('');
      } else if (activeField === 'modelName') {
        setModelName((v) => backspaceByGraphemeApprox(v));
        onError('');
      }
      return;
    }

    if (!input) return;
    if (key.ctrl || key.meta) return;

    if (activeField === 'providerName') {
      setProviderName((v) => v + input);
      setProviderNameTouched(true);
      onError('');
    } else if (activeField === 'baseUrl') {
      setBaseUrl((v) => v + input);
      setBaseUrlTouched(true);
      onError('');
    } else if (activeField === 'apiKey') {
      setApiKey((v) => v + input);
      onError('');
    } else if (activeField === 'modelName') {
      setModelName((v) => v + input);
      onError('');
    }
  });

  if (providerPhase === 'brand') {
    return (
      <Box flexDirection="column" marginTop={1}>
        <SelectInput
          items={providerBrandItems}
          onSelect={(item) => {
            const brand = String(item.value);
            setProviderBrand(brand);
            applyPreset(brand);
            setProviderPhase('edit');
            setActiveField('providerName');
            onError('');
          }}
        />
      </Box>
    );
  }

  const labelWidth = Math.max(
    getCachedStringWidth('provider_id'),
    getCachedStringWidth('base_url'),
    getCachedStringWidth('api_key'),
    getCachedStringWidth('model_name'),
    getCachedStringWidth(t('welcome_wizard.labels.next')),
  );

  const renderField = (label: string, rawValue: string, k: FieldKey, masked?: boolean) => {
    const active = activeField === k;
    const labelPadded = padLeftToWidth(label, labelWidth);
    const prefix = `${active ? '> ' : '  '}${labelPadded}: `;
    const display = masked ? maskSecret(rawValue) : rawValue;
    const cursorChar = '█';
    const cursor = active ? cursorChar : '';
    const cursorWidth = active ? getCachedStringWidth(cursorChar) : 0;
    const valueMax = Math.max(0, inputWidth - cursorWidth);
    const valueTrimmed = truncateToWidth(display, valueMax);
    const line = `${prefix}${padRightToWidth(valueTrimmed + cursor, inputWidth)}`;
    const underline = `${' '.repeat(getCachedStringWidth(prefix))}${'─'.repeat(inputWidth)}`;
    return (
      <Box flexDirection="column">
        <Text color={active ? themeColors.success : themeColors.text}>{line}</Text>
        <Text dimColor>{underline}</Text>
      </Box>
    );
  };

  const nextLine = () => (
    <Text color={activeField === 'next' ? themeColors.success : themeColors.text}>
      {(activeField === 'next' ? '> ' : '  ') + padLeftToWidth(t('welcome_wizard.labels.next'), labelWidth) + '  '}
    </Text>
  );

  return (
    <Box flexDirection="column" marginTop={1}>
      {renderField('provider_id', providerName, 'providerName')}
      {renderField('base_url', baseUrl, 'baseUrl')}
      {renderField('api_key', apiKey, 'apiKey', true)}
      {renderField('model_name', modelName, 'modelName')}
      {nextLine()}
    </Box>
  );
}

function ReviewStepContent(props: {
  ctx: StepContext;
  language: string;
  selectedTheme: string;
  agentMode: string;
  providerName: string;
  providerBrand: string;
  baseUrl: string;
  apiKey: string;
  modelName: string;
  submitting: boolean;
  onSubmit: () => void;
  onTitle: (title: string, subtitle?: string) => void;
  onHint: (hint: string) => void;
}) {
  const { t } = useTranslation();
  const { ctx, language, selectedTheme, agentMode, providerName, providerBrand, baseUrl, apiKey, modelName, submitting, onSubmit } =
    props;
  const { onTitle, onHint } = props;

  useEffect(() => {
    onTitle(t('welcome_wizard.titles.review'));
    onHint(t('welcome_wizard.hints.review'));
  }, [onHint, onTitle, t]);

  useInput((_, key) => {
    if (key.escape) ctx.goBack();
    if (key.return && !submitting) {
      onSubmit();
    }
  });

  return (
    <Box flexDirection="column" marginTop={1}>
      <Text>
        {t('welcome_wizard.labels.language')}: {language}
      </Text>
      <Text>
        {t('welcome_wizard.labels.theme')}: {selectedTheme}
      </Text>
      <Text>
        {t('welcome_wizard.labels.agent')}: {t(`agent_mode.${agentMode}`)}
      </Text>
      <Text>
        {t('welcome_wizard.labels.provider')}: {providerName} ({providerBrand})
      </Text>
      <Text>base_url: {baseUrl}</Text>
      <Text>api_key: {maskSecret(apiKey)}</Text>
      <Text>model_name: {modelName}</Text>
      <Text dimColor>{submitting ? t('status.submitting') : ''}</Text>
    </Box>
  );
}

export function WelcomeConfigWizard({ sessionId, onFinished, onCancelled }: WelcomeConfigWizardProps) {
  const { t } = useTranslation();
  const { theme, availableThemes, applyThemeName, setThemeName } = useTheme();
  const { stdout } = useStdout();
  const columns = stdout?.columns ?? 80;
  const contentWidth = Math.max(20, columns - 2 - 4);
  const {
    listProviderPresets,
    setLanguage,
    setWelcomeWizardDone,
    setAgentMode,
    saveUserProviders,
    reloadSessionConfig,
    setModel,
  } = useRustBridge();

  const presets = useMemo(() => {
    try {
      return listProviderPresets() as ProviderPreset[];
    } catch {
      return [];
    }
  }, []);

  const providerBrandItems = useMemo(() => {
    const fallback: ProviderPreset[] =
      presets.length > 0
        ? presets
        : [
            {
              providerId: 'openai',
              providerBrand: 'OpenAI',
              baseUrl: 'https://api.openai.com/v1',
              apiKey: 'OPENAI_API_KEY',
              modelName: 'gpt-4o',
              providerDesc: 'OpenAI GPT models',
            },
          ];
    return fallback.map((p) => ({ label: p.providerBrand, value: p.providerId }));
  }, [presets]);

  const themeItems = useMemo(() => availableThemes.map((t) => ({ label: t.name, value: t.name })), [availableThemes]);

  const languageItems = useMemo(
    () => [
      { label: 'English (en)', value: 'en' as LanguageOption },
      { label: 'Chinese (Simplified, zh-CN)', value: 'zh-CN' as LanguageOption },
    ],
    [],
  );

  const agentItems = useMemo(
    () => [
      { label: t('agent_mode.build'), value: 'build' as AgentModeOption },
      { label: t('agent_mode.plan'), value: 'plan' as AgentModeOption },
    ],
    [t],
  );

  const [language, setLanguageState] = useState<LanguageOption>('zh-CN');
  const [selectedTheme, setSelectedTheme] = useState<string>(theme.name);
  const [agentMode, setAgentModeState] = useState<AgentModeOption>('build');

  const [providerBrand, setProviderBrand] = useState<string>('');
  const [providerName, setProviderName] = useState<string>('');
  const [baseUrl, setBaseUrl] = useState<string>('');
  const [apiKey, setApiKey] = useState<string>('');
  const [modelName, setModelName] = useState<string>('');

  const [providerNameTouched, setProviderNameTouched] = useState(false);
  const [baseUrlTouched, setBaseUrlTouched] = useState(false);

  const [providerPhase, setProviderPhase] = useState<'brand' | 'edit'>('brand');
  const [activeField, setActiveField] = useState<FieldKey>('providerName');

  const [submitting, setSubmitting] = useState(false);
  const [errorText, setErrorText] = useState<string>('');
  const [hintText, setHintText] = useState<string>(() => t('welcome_wizard.hints.list'));
  const [stepTitle, setStepTitle] = useState<string>(() => t('welcome_wizard.titles.language'));
  const [stepSubtitle, setStepSubtitle] = useState<string>('');

  const getPreset = (name: string): ProviderPreset | undefined =>
    presets.find((p) => p.providerId === name);

  const applyPreset = (name: string) => {
    const preset = getPreset(name);
    if (!preset) return;

    if (!providerNameTouched || providerName.trim().length === 0) {
      setProviderName(preset.providerId);
    }
    if (!baseUrlTouched || baseUrl.trim().length === 0) {
      setBaseUrl(preset.baseUrl);
    }
    // We populate modelName if it's empty
    if (modelName.trim().length === 0) {
        setModelName(preset.modelName);
    }
    // Note: We do NOT autofill apiKey with the env var name, leaving it empty for user to fill
  };

  const persistAll = async () => {
    setSubmitting(true);
    setErrorText('');
    try {
      logger.info(
        `welcomeWizard.submit start lang=${language} theme=${selectedTheme} agentMode=${agentMode} providerBrand=${providerBrand} providerName=${providerName} baseUrl=${baseUrl} modelName=${modelName} apiKey=${maskSecret(
          apiKey,
        )}`,
      );

      const languageTrimmed = language.trim();
      if (languageTrimmed) {
        i18n.changeLanguage(languageTrimmed).catch(() => {});
        await setLanguage(languageTrimmed);
        logger.info(`welcomeWizard.submit setLanguage ok lang=${languageTrimmed}`);
      }

      if (selectedTheme) {
        setThemeName(selectedTheme);
        logger.info(`welcomeWizard.submit setTheme ok theme=${selectedTheme}`);
      }
      logger.info(`welcomeWizard.submit validate start`);

      const pn = providerName.trim();
      const mn = modelName.trim();
      const b = providerBrand.trim();
      const u = baseUrl.trim();
      const k = apiKey;

      if (!b || !pn || !mn || !u || !k) {
        throw new Error(t('welcome_wizard.errors.provider_incomplete'));
      }
      if (!isLikelyUrl(u)) {
        throw new Error(t('welcome_wizard.errors.base_url_invalid'));
      }

      logger.info(`welcomeWizard.submit saveUserProviders start providerName=${pn} modelName=${mn} baseUrl=${u}`);
      await saveUserProviders([
        {
          providerBrand: b,
          providerId: pn,
          modelName: mn,
          baseUrl: u,
          apiKey: k,
        },
      ]);
      logger.info(`welcomeWizard.submit saveUserProviders ok`);

      await setWelcomeWizardDone(true);
      logger.info(`welcomeWizard.submit setWelcomeWizardDone ok`);

      await reloadSessionConfig(sessionId);
      logger.info(`welcomeWizard.submit reloadSessionConfig ok sessionId=${sessionId}`);

      await setAgentMode(sessionId, agentMode);
      logger.info(`welcomeWizard.submit setAgentMode ok mode=${agentMode}`);

      await setModel(sessionId, pn, mn);
      logger.info(`welcomeWizard.submit setModel ok provider=${pn} model=${mn}`);

      onFinished();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      logger.error(`welcomeWizard.submit failed: ${msg}`);
      setErrorText(msg);
      setSubmitting(false);
    }
  };

  const footer = useMemo(() => {
    return (
      <Box flexDirection="column">
        <Text dimColor>{hintText}</Text>
        {errorText && <Text color={theme.colors.warning}>{errorText}</Text>}
      </Box>
    );
  }, [errorText, hintText, theme.colors.primary, theme.colors.warning]);

  return (
    <Box
      flexDirection="column"
      width="100%"
      borderStyle="round"
      borderColor={theme.colors.border}
      paddingX={2}
      paddingY={1}
    >
      <Stepper
        keyboardNav={false}
        showProgress
        renderProgress={(ctx) => (
          <Box flexDirection="column" width="100%">
            <WizardProgressBar
              progress={ctx}
              width={contentWidth}
              colors={{ success: theme.colors.success, text: theme.colors.text }}
              labelForStepName={(name) => {
                const key = String(name ?? '').toLowerCase();
                if (key === 'language') return t('welcome_wizard.progress.language');
                if (key === 'theme') return t('welcome_wizard.progress.theme');
                if (key === 'agent') return t('welcome_wizard.progress.agent');
                if (key === 'provider') return t('welcome_wizard.progress.provider');
                if (key === 'review') return t('welcome_wizard.progress.review');
                return name;
              }}
            />
            <Box marginTop={1}>
              <WizardTitleArea
                title={stepTitle}
                subtitle={stepSubtitle || undefined}
                colors={{ primary: theme.colors.primary }}
              />
            </Box>
          </Box>
        )}
        onComplete={() => {}}
        onCancel={onCancelled}
      >
        <Step name="Language">
          {(ctx) => (
            <LanguageStepContent
              ctx={ctx}
              languageItems={languageItems}
              onSelect={(next) => {
                setLanguageState(next);
                i18n.changeLanguage(next).catch(() => {});
              }}
              onTitle={(title, subtitle) => {
                setStepTitle(title);
                setStepSubtitle(subtitle ?? '');
              }}
              onHint={setHintText}
            />
          )}
        </Step>

        <Step name="Theme">
          {(ctx) => (
            <ThemeStepContent
              ctx={ctx}
              themeItems={themeItems}
              onSelect={(next) => {
                setSelectedTheme(next);
                applyThemeName(next);
              }}
              onTitle={(title, subtitle) => {
                setStepTitle(title);
                setStepSubtitle(subtitle ?? '');
              }}
              onHint={setHintText}
            />
          )}
        </Step>

        <Step name="Agent">
          {(ctx) => (
            <AgentStepContent
              ctx={ctx}
              agentItems={agentItems}
              onSelect={setAgentModeState}
              onTitle={(title, subtitle) => {
                setStepTitle(title);
                setStepSubtitle(subtitle ?? '');
              }}
              onHint={setHintText}
            />
          )}
        </Step>

        <Step name="Provider">
          {(ctx) => (
            <ProviderStepContent
              ctx={ctx}
              themeColors={{ success: theme.colors.success, text: theme.colors.text }}
              inputWidth={Math.max(20, Math.min(72, columns - 24))}
              providerBrandItems={providerBrandItems}
              providerBrand={providerBrand}
              setProviderBrand={setProviderBrand}
              providerPhase={providerPhase}
              setProviderPhase={setProviderPhase}
              activeField={activeField}
              setActiveField={setActiveField}
              providerName={providerName}
              setProviderName={setProviderName}
              setProviderNameTouched={setProviderNameTouched}
              baseUrl={baseUrl}
              setBaseUrl={setBaseUrl}
              setBaseUrlTouched={setBaseUrlTouched}
              apiKey={apiKey}
              setApiKey={setApiKey}
              modelName={modelName}
              setModelName={setModelName}
              applyPreset={applyPreset}
              onTitle={(title, subtitle) => {
                setStepTitle(title);
                setStepSubtitle(subtitle ?? '');
              }}
              onHint={setHintText}
              onError={setErrorText}
            />
          )}
        </Step>

        <Step name="Review">
          {(ctx) => (
            <ReviewStepContent
              ctx={ctx}
              language={language}
              selectedTheme={selectedTheme}
              agentMode={agentMode}
              providerName={providerName}
              providerBrand={providerBrand}
              baseUrl={baseUrl}
              apiKey={apiKey}
              modelName={modelName}
              submitting={submitting}
              onTitle={(title, subtitle) => {
                setStepTitle(title);
                setStepSubtitle(subtitle ?? '');
              }}
              onHint={setHintText}
              onSubmit={() => {
                persistAll().catch(() => {});
              }}
            />
          )}
        </Step>
      </Stepper>
      <Box marginTop={1}>{footer}</Box>
    </Box>
  );
}
