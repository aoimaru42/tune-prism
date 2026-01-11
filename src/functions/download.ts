import { save } from '@tauri-apps/api/dialog'
import { invoke } from '@tauri-apps/api/tauri'

/**
 * stemファイルをZIPファイルとしてダウンロード（保存ダイアログを表示して保存）
 * バックエンドでZIPファイルを作成してからダウンロード
 */
export async function downloadStemsAsZip(projectId: string, stemPaths: string[]): Promise<void> {
    try {
        if (stemPaths.length === 0) {
            throw new Error('ダウンロードするstemがありません')
        }

        // 日付をyyyymmdd形式で取得
        const now = new Date()
        const year = now.getFullYear()
        const month = String(now.getMonth() + 1).padStart(2, '0')
        const day = String(now.getDate()).padStart(2, '0')
        const dateStr = `${year}${month}${day}`
        
        // 連番を生成（タイムスタンプのミリ秒部分を使用）
        const timestamp = now.getTime()
        const sequence = String(timestamp % 10000).padStart(4, '0')
        
        // ファイル名を生成: tune_stem_yyyymmdd連番.zip
        const fileName = `tune_stem_${dateStr}_${sequence}.zip`

        // 保存ダイアログを表示
        const filePath = await save({
            title: 'StemsをZIPファイルとして保存',
            defaultPath: fileName,
            filters: [{
                name: 'ZIP',
                extensions: ['zip']
            }]
        })

        if (!filePath) {
            // ユーザーがキャンセルした場合
            return
        }

        // バックエンドでZIPファイルを作成
        await invoke('create_stems_zip', {
            projectId,
            stemPaths,
            outputPath: filePath
        })
        
        console.log(`Stems saved to ZIP: ${filePath}`)
    } catch (error) {
        console.error('Error downloading stems as ZIP:', error)
        throw error
    }
}
