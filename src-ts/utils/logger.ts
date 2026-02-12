import winston from 'winston';
import path from 'path';
import fs from 'fs';

// Define log levels and colors if needed
const logFormat = winston.format.printf(function({ level, message, timestamp }) {
  return `[${timestamp}] ${level.toUpperCase()}: ${message}`;
});

const logDir = path.join(process.cwd(), 'logs');
try {
  fs.mkdirSync(logDir, { recursive: true });
} catch {
}

const defaultLevel = process.env.CARRYCODE_LOG_LEVEL
    ? String(process.env.CARRYCODE_LOG_LEVEL)
    : process.env.CARRYCODE_DEBUG_EVENTS
      ? 'debug'
      : 'info';

export const logger = winston.createLogger({
  level: defaultLevel,
  format: winston.format.combine(
    winston.format.timestamp(),
    logFormat
  ),
  transports: [
    // Write all logs with level 'info' and below to carrycode-ts.log
    new winston.transports.File({ 
      filename: path.join(logDir, 'carrycode-ts.log'),
      level: 'debug' 
    }),
    // Write all errors to error.log
    new winston.transports.File({ 
      filename: path.join(logDir, 'carrycode-ts.log'), 
      level: 'error' 
    }),
  ],
});

// If we're not in production, we could also log to console, 
// but since this is a TUI app, console logs might interfere with Ink UI.
// So we generally keep it to file transports only.
