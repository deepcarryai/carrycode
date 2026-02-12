import React, { useMemo } from 'react';
import { Box, Text, Static } from 'ink';
import type { Message } from '../types/index.js';
import { RichArea } from './RichArea.js';
import { WelcomeBanner } from './WelcomeBanner.js';

interface OutputAreaProps {
  messages: Message[];
  staticEpoch?: number;
}

interface StaticItem {
  id: string;
  type: 'question' | 'segment';
  message: Message;
  segmentIndex?: number;
}

export const OutputArea: React.FC<OutputAreaProps> = ({ messages, staticEpoch = 0 }) => {
  const staticItems = useMemo(() => {
    const items: StaticItem[] = [];
    for (let msgIndex = 0; msgIndex < messages.length; msgIndex++) {
      const msg = messages[msgIndex];
      items.push({
        id: `q-${msgIndex}-${msg.startTime ?? msgIndex}`,
        type: 'question',
        message: msg,
      });
      for (let segIndex = 0; segIndex < msg.segments.length; segIndex++) {
        const seg = msg.segments[segIndex];
        items.push({
          id: `s-${msgIndex}-${segIndex}-${seg.stage}-${seg.title}`,
          type: 'segment',
          message: msg,
          segmentIndex: segIndex,
        });
      }
    }
    return items;
  }, [messages]);

  const renderItem = (item: StaticItem) => {
    if (item.type === 'question') {
      if (!item.message.question) return null;
      return (
        <Box key={item.id} marginBottom={1} paddingLeft={1}>
          <Text>{'>'} {item.message.question}</Text>
        </Box>
      );
    }

    const segment =
      item.segmentIndex !== undefined ? item.message.segments[item.segmentIndex] : undefined;
    if (!segment) return null;

    if (segment.isBanner) {
      return (
        <Box key={item.id} marginBottom={1}>
          <WelcomeBanner />
        </Box>
      );
    }

    const displayTitle = segment.title;
    return (
      <Box key={item.id} marginBottom={1}>
        <RichArea
          content={segment.content}
          title={displayTitle}
          tools={segment.tools}
          mode={segment.toolOperation ? 'tool' : 'text'}
          toolOperation={segment.toolOperation}
          isDone={true}
        />
      </Box>
    );
  };

  return (
    <Box flexDirection="column" paddingBottom={1}>
      <Static key={`static-${staticEpoch}`} items={staticItems}>
        {renderItem}
      </Static>
    </Box>
  );
};
