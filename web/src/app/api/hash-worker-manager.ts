// hash-worker-manager.ts
import {createSHA256} from "hash-wasm";

export class HashWorkerManager {
    private worker: Worker | null = null;
    private readonly chunkSize = 1024 * 1024; // 1MB

    constructor() {
        if (typeof Worker !== 'undefined') {
            // 创建Web Worker
            this.worker = new Worker(new URL('./hash.worker', import.meta.url));
        }
    }

    slice(chunkNumber: number, blob: Blob): Blob {
        const start = (chunkNumber - 1) * this.chunkSize;
        const end = Math.min(start + this.chunkSize, blob.size);
        return blob.slice(start, end);
    }

    async postChunkToWorker(worker: Worker, chunk: Uint8Array<ArrayBuffer>): Promise<void> {
        return new Promise((resolve, reject) => {
            const handler = (e: MessageEvent) => {
                if(e.data.type === 'UPDATE_FILE_CHUNK_RESULT'){
                    this.worker?.removeEventListener('message', handler);
                    if (e.data.success) {
                        resolve();
                    } else {
                        reject(new Error(e.data.error));
                    }
                }
            }

            worker.addEventListener('message', handler);

            worker.postMessage({
                type: 'UPDATE_FILE_CHUNK', data: chunk,
            });
        });
    }

    async postGetHashMessageToWorker(worker: Worker): Promise<string> {
        return new Promise((resolve, reject) => {
            const handler = (e: MessageEvent) => {
                if(e.data.type === 'GET_HASH_RESULT'){
                    this.worker?.removeEventListener('message', handler);
                    if (e.data.success) {
                        resolve(e.data.hash);
                    } else {
                        reject(new Error(e.data.error));
                    }
                }
            }

            worker.addEventListener('message', handler);

            worker.postMessage({
                type: 'GET_HASH',
            });
        });
    }


    // 计算文件哈希
    async calculateHash(blob: Blob): Promise<string> {
        if (!this.worker) {
            // 备用方案：在主线程中计算（不推荐，仅作降级处理）
            return this.calculateHashFallback(blob);
        }

        const totalChunks = Math.ceil(blob.size / this.chunkSize);
        for (let i = 0; i < totalChunks; i++) {
            const chunkNumber = i + 1;
            const chunk = this.slice(chunkNumber, blob);
            const buffer = await chunk.arrayBuffer();
            const uint8Array = new Uint8Array(buffer);
            await this.postChunkToWorker(this.worker, uint8Array);
        }

        return await this.postGetHashMessageToWorker(this.worker);
    }

    // 降级方案：在主线程中计算哈希
    private async calculateHashFallback(blob: Blob): Promise<string> {
        console.log('进入降级方案：在主线程中计算哈希');
        const totalChunks = Math.ceil(blob.size / this.chunkSize);

        const hasher = await createSHA256();
        for (let i = 0; i < totalChunks; i++) {
            const chunkNumber = i + 1;
            const chunk = this.slice(chunkNumber, blob);
            const buffer = await chunk.arrayBuffer();
            const uint8Array = new Uint8Array(buffer);
            hasher.update(uint8Array)
        }
        return hasher.digest('hex');
    }

    // 清理Worker
    terminate() {
        this.worker?.terminate();
        this.worker = null;
    }
}