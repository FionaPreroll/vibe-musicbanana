// Numbers and dates in the viewer's locale.

const number = new Intl.NumberFormat();
const date = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' });
const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });

export const formatNumber = (n: number) => number.format(n);
export const formatDate = (iso: string) => date.format(new Date(iso));
export const formatDateTime = (iso: string) => dateTime.format(new Date(iso));

export const listenCount = (n: number) => `${number.format(n)} ${n === 1 ? 'listen' : 'listens'}`;
