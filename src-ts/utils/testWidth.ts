// Test to verify string width calculations
import { getCachedStringWidth, padLeftToWidth } from './textUtils.js';

// Test cases
console.log('Testing string width calculations:');
console.log('ASCII: "provider_id" width:', getCachedStringWidth('provider_id'), 'length:', 'provider_id'.length);
console.log('Chinese: "下一步" width:', getCachedStringWidth('下一步'), 'length:', '下一步'.length);
console.log('Mixed: "model_name" width:', getCachedStringWidth('model_name'), 'length:', 'model_name'.length);

// Test padding
const labels = ['provider_id', 'base_url', 'api_key', 'model_name', '下一步'];
const labelWidth = Math.max(...labels.map(l => getCachedStringWidth(l)));

console.log('\nMax label width:', labelWidth);
console.log('\nPadded labels:');
labels.forEach(label => {
  const padded = padLeftToWidth(label, labelWidth);
  console.log(`"${label}" -> "${padded}"`);
});
