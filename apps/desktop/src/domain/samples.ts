// Sample antennas shipped with MMANA (<repo>/testdata/maa), bundled as assets.
const urls = import.meta.glob('../../../../testdata/maa/*.MAA', {
  query: '?url',
  import: 'default',
  eager: true,
}) as Record<string, string>;

export interface Sample {
  file: string;
  url: string;
}

export const samples: Sample[] = Object.entries(urls)
  .map(([path, url]) => ({ file: path.split('/').pop() ?? path, url }))
  .sort((a, b) => a.file.localeCompare(b.file, 'en', { numeric: true }));

export async function loadSampleBytes(sample: Sample): Promise<Uint8Array> {
  const res = await fetch(sample.url);
  return new Uint8Array(await res.arrayBuffer());
}
