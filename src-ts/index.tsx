#!/usr/bin/env node
import React from 'react';
import { render } from 'ink';
import i18n from './i18n/index.js';
import { App } from './components/App.js';
import { loadCoreApi } from './utils/loadCoreApi.js';
import { logger } from './utils/logger.js';

// Inject via build script define
declare const CARRYCODE_VERSION: string;

function parseOnceArgs(argv: string[]): { prompt: string; timeoutMs: number } | null {
  // Handle Version
  if (argv.includes('--version') || argv.includes('-v')) {
  console.log(`carry-cli version ${typeof CARRYCODE_VERSION !== 'undefined' ? CARRYCODE_VERSION : '0.0.0-dev'}`);
    process.exit(0);
  }

  // Handle Help
  if (argv.includes('--help') || argv.includes('-h')) {
    console.log(`
Usage: carry [options]

Options:
  -v, --version        Output the version number
  -h, --help           Output usage information
  --once <prompt>      Run a single prompt and exit
  --timeout-ms <ms>    Set timeout for --once mode (default: 15000ms)

Interactive Mode:
  Run 'carry' without arguments to start the interactive TUI.
`);
    process.exit(0);
  }

  const idxOnce = argv.indexOf('--once');
  if (idxOnce < 0) return null;

  const idxTimeout = argv.indexOf('--timeout-ms');
  const timeoutMsRaw = idxTimeout >= 0 ? argv[idxTimeout + 1] : undefined;
  const timeoutMs = timeoutMsRaw ? Number(timeoutMsRaw) : 15000;
  const safeTimeoutMs = Number.isFinite(timeoutMs) && timeoutMs > 0 ? timeoutMs : 15000;

  const idxPrompt = argv.indexOf('--prompt');
  if (idxPrompt >= 0 && argv[idxPrompt + 1]) {
    return { prompt: argv[idxPrompt + 1]!, timeoutMs: safeTimeoutMs };
  }

  const remaining = argv.filter(
    (a, i) =>
      i !== idxOnce &&
      i !== idxTimeout &&
      i !== idxTimeout + 1 &&
      a !== '--once' &&
      a !== '--prompt' &&
      a !== '--timeout-ms',
  );
  if (remaining.length > 0) {
    return { prompt: remaining.join(' '), timeoutMs: safeTimeoutMs };
  }

  return { prompt: '', timeoutMs: safeTimeoutMs };
}

async function runOnce(prompt: string, timeoutMs: number) {
  const coreapi = loadCoreApi();
  const sessionId = coreapi.createSessionId();
  const session = await coreapi.Session.open(sessionId);
  let sawConfirmation = false;
  const debugEvents = process.env.CARRYCODE_DEBUG_EVENTS === '1';
  let eventCount = 0;
  let nullEventCount = 0;
  const timeout = setTimeout(() => {
    try {
      process.stderr.write(`execute timeout after ${timeoutMs}ms\n`);
    } finally {
      try {
        session.unsubscribe();
      } catch {
      }
      process.exit(124);
    }
  }, timeoutMs);

  session.subscribe(
    (err: any, event: any) => {
      if (err) {
        if (debugEvents) {
          logger.warn(`once stream err session=${sessionId} err=${String(err?.message ?? err)}`);
        }
        return;
      }
      if (!event) {
        nullEventCount += 1;
        return;
      }
      const eventType = event?.eventType;
      if (eventType === 'Text' && typeof event.text === 'string') {
        process.stdout.write(event.text);
      }
      if (debugEvents) {
        eventCount += 1;
        if (eventCount <= 50) {
          const textLen = typeof event.text === 'string' ? event.text.length : 0;
          logger.debug(
            `once#${eventCount} session=${sessionId} type=${String(eventType)} stage=${String(
              event.stage ?? '',
            )} op=${String(event.toolOperation ?? '')} textLen=${textLen}`,
          );
        }
      }
    },
    (err: any, event: any) => {
      if (err) {
        if (debugEvents) {
          logger.warn(`once control err session=${sessionId} err=${String(err?.message ?? err)}`);
        }
        return;
      }
      if (!event) {
        nullEventCount += 1;
        return;
      }
      const eventType = event?.eventType;
      if (eventType === 'ToolOutput') {
        const text = typeof event.displayText === 'string' ? event.displayText : event.responseSummary;
        if (typeof text === 'string' && text.length > 0) {
          process.stdout.write(text.endsWith('\n') ? text : text + '\n');
        }
      }
      if (eventType === 'ConfirmationRequested' && event.confirm?.requestId) {
        if (!sawConfirmation) {
          sawConfirmation = true;
        }
        void session.confirmTool({ requestId: event.confirm.requestId, decision: '3' });
      }
      if (debugEvents) {
        eventCount += 1;
        if (eventCount <= 50) {
          const textLen = typeof event.text === 'string' ? event.text.length : 0;
          logger.debug(
            `once#${eventCount} session=${sessionId} type=${String(eventType)} stage=${String(
              event.stage ?? '',
            )} op=${String(event.toolOperation ?? '')} textLen=${textLen}`,
          );
        }
      }
    },
  );

  try {
    if (debugEvents) {
      logger.info(`once subscribe session=${sessionId} promptChars=${prompt.length}`);
    }
    const result: any = await session.execute(prompt);
    if (typeof result?.content === 'string' && result.content.trim().length > 0) {
      if (!result.content.endsWith('\n')) {
        process.stdout.write('\n');
      }
    }
    if (sawConfirmation) {
      process.exit(2);
    }
  } catch (e) {
    process.stderr.write(String(e instanceof Error ? e.message : e) + '\n');
    process.exit(1);
  } finally {
    clearTimeout(timeout);
    try {
      session.unsubscribe();
    } catch {
    }
    if (debugEvents) {
      logger.info(`once unsubscribe session=${sessionId} events=${eventCount} nullEvents=${nullEventCount}`);
    }
  }
}

const onceArgs = parseOnceArgs(process.argv.slice(2));
if (onceArgs !== null) {
  void runOnce(onceArgs.prompt, onceArgs.timeoutMs);
} else {
  const startInteractive = async () => {
    try {
      const coreapi = loadCoreApi();
      const state: any = coreapi.getConfigBootstrapState?.();
      const lang = String(state?.runtimeLanguage ?? '').trim();
      if (lang) {
        await i18n.changeLanguage(lang);
      }
    } catch {
    }
    render(<App />, { exitOnCtrlC: false });
  };

  void startInteractive();
}
