
export interface ShareItem {
    id: string,
    file_name: string,
    mime_type: string,
}

export type ShareList = Array<ShareItem>;

export async function getShareList(): Promise<ShareList> {
    const response = await fetch('/shares', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
    });
    if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
    }
    return await response.json() as ShareList;
}

