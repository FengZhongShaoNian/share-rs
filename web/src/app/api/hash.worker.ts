// hash.worker.ts

import {createSHA256, IHasher} from "hash-wasm";

interface HasherHolder {
    hasher: IHasher | null;
}

const hashWorker: HasherHolder = {
    hasher: null
};

// 监听主线程消息
self.onmessage = async function (e) {
    const {type, data} = e.data;

    if (type === 'UPDATE_FILE_CHUNK') {
        try {
            if(!hashWorker.hasher){
                console.log('初始化hasher');
                hashWorker.hasher = await createSHA256();
            }

            const chunk: Uint8Array<ArrayBuffer> = data;
            hashWorker.hasher!.update(chunk);
            // 发送结果回主线程
            self.postMessage({
                type: 'UPDATE_FILE_CHUNK_RESULT', success: true
            });
        } catch (error) {
            console.error(error);
            // 发送结果回主线程
            self.postMessage({
                type: 'UPDATE_FILE_CHUNK_RESULT', success: false, error: error
            });
        }
    } else if (type === 'GET_HASH') {
        try {
            const hash = hashWorker.hasher!.digest('hex');
            self.postMessage({
                type: 'GET_HASH_RESULT', success: true, hash: hash
            });
        } catch (error) {
            console.error(error);
            self.postMessage({
                type: 'GET_HASH_RESULT', success: false, error: error
            });
        }

    }
};