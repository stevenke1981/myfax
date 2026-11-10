# Epson 印表機設定指南（繁體中文）

本文件說明如何設定具傳真功能的 Epson 印表機，讓收到的傳真存到網路資料夾，供 MyFax 接收。
English version: [below](#epson-printer-setup-guide-english).

## 適用範圍與注意事項

- 適用於具傳真功能的 Epson 印表機，例如 WorkForce 或 EcoTank L 系列（例如 L5590）。
- 選單名稱依機型與韌體而異。若找不到下列名稱，請尋找「Fax Output」（傳真輸出）或「儲存到網路資料夾」相關選項。
- MyFax 不會直接與印表機通訊，也不會取代 Epson 驅動程式。MyFax 只監看您設定的資料夾。
- 請勿讓 EPSON FAX Utility 與 MyFax 同時監看同一個資料夾。
- MyFax 只監看資料夾，**不會**接收以電子郵件轉寄的傳真。

## 步驟一：在電腦上建立共用資料夾

1. 建立一個資料夾，例如 `C:\FaxInbox`。
2. 將該資料夾設為網路共用，共用名稱例如 `FaxInbox`。
3. 授予印表機登入網路時所使用的帳號寫入權限。共用與權限的操作步驟依 Windows 版本與網路環境而異。請僅授予必要的權限。

確認共用成功：在另一台電腦的檔案總管輸入 `\\PC-NAME\FaxInbox`，應能開啟資料夾並建立檔案。

## 步驟二：設定印表機

1. 在瀏覽器輸入印表機的 IP 位址，開啟 Web Config；或使用印表機的操作面板。進入傳真的「傳真輸出」設定。
2. 將「傳真輸出」設為「儲存到網路資料夾」（SMB），路徑填入這台電腦上的共用資料夾，例如 `\\PC-NAME\FaxInbox`。
3. 檔案格式建議選 **PDF**。若選 TIFF，請在 MyFax 中開啟「同時接受 TIFF 傳真」。
4. 將設定儲存。

## 步驟三：在 MyFax 中加入監看資料夾

1. 開啟 MyFax，進入「設定」頁的「接收」區塊。
2. 在「監看資料夾」輸入與印表機相同的資料夾路徑，然後按「新增」。
3. 依需要調整「掃描間隔（秒）」（預設 5 秒）。
4. 若印表機存成 TIFF，開啟「同時接受 TIFF 傳真（.tif / .tiff）」。
5. 按狀態列的「立即掃描」進行測試。

## 步驟四：測試

從另一部傳真機傳送一份測試傳真。等候印表機寫入檔案後，MyFax 會掃描資料夾並匯入。若沒有出現，請參考下方的疑難排解。

## MyFax 如何處理收到的檔案

- **等待寫入完成**：檔案大小與修改時間在兩次掃描之間都沒有變化、檔案已存在至少 5 秒，且 PDF 結尾有 `%%EOF`（TIFF 需有正確檔頭），才會匯入；沒有結尾標記的檔案在連續 10 次不變後仍會匯入。
- **監看資料夾規則**：必須是完整路徑，且不可位於封存根目錄內、也不可與「額外複製資料夾」相同或位於其中。
- **只讀取最上層**：監看資料夾只讀取最上層的檔案，不會進入子資料夾。
- **封存位置**：複製到 `封存根目錄\YYYY\YYYY-MM\YYYY-MM-DD\YYYYMMDD-HHMMSS_寄件號碼.pdf`。沒有寄件號碼時，寄件號碼部分為 `unknown`。
- **檔名解析範例**：`FAX_20261110_103045_0912345678.pdf` 會解析為 2026-11-10 10:30:45、寄件號碼 0912345678。若檔名中找不到有效日期，使用檔案的修改時間。
- **重複內容**：以 SHA-256 比對。相同內容的檔案只會封存一次，活動記錄會顯示「略過重複的傳真」。
- **來源檔案**：預設保留。若開啟「封存後刪除來源檔案（移動而非複製）」，匯入成功後會刪除來源檔案。
- **額外複製**：若設定了「額外複製到資料夾」，每份新傳真會另外複製一份到該資料夾。

## 傳送傳真（選用）

傳送功能使用電腦上已安裝的 **EPSON FAX** 傳真驅動程式：

1. 安裝 Epson FAX 驅動程式（印表機名稱通常含有「FAX」）。
2. 在 MyFax 的「傳送傳真」頁按「重新整理」，選擇傳真印表機。
3. 選擇要傳送的檔案，填寫收件人（選填），然後按「傳送傳真」。
4. MyFax 會把文件交給驅動程式，由驅動程式跳出視窗。請在該視窗輸入傳真號碼並送出。

MyFax 無法代為撥號，也無法確認對方是否收到。送達與否由驅動程式與印表機負責。

## 疑難排解

| 現象 | 可能原因 | 處理方式 |
|---|---|---|
| 「找不到資料夾：…」訊息，或狀態列顯示監看錯誤 | 路徑錯誤，或共用無法存取 | 在檔案總管開啟該路徑。修正路徑或共用權限後，按「立即掃描」。 |
| 印表機顯示已傳送，但 MyFax 沒有反應 | 資料夾未加入監看清單；檔案不是 PDF（或 TIFF 未開啟）；檔案仍在寫入或為 0 位元組 | 檢查監看清單與 TIFF 開關。稍候後按「立即掃描」。 |
| 印表機無法寫入共用資料夾 | 共用權限不足，或帳號與印表機設定不符 | 確認印表機使用的帳號對該資料夾有寫入權限。 |
| 同一份傳真只出現一次 | 內容重複，已被略過 | 屬正常行為。 |
| 收到的 TIFF 無法預覽 | TIFF 的非第一頁使用非 CCITT 壓縮 | 按「開啟」以預設程式檢視。 |
| 「傳送傳真」找不到印表機 | 未安裝 EPSON FAX 驅動程式，或清單未更新 | 安裝驅動程式後，按「重新整理」。 |
| 印表機選單找不到「Fax Output」 | 機型或韌體的選單名稱不同 | 尋找「儲存到網路資料夾」或其他含「Save to Network Folder」的選項；若仍找不到，請查閱該機型手冊。 |

---

# Epson Printer Setup Guide (English)

This guide explains how to configure an Epson printer with fax so that received faxes are saved to a
network folder that MyFax watches.
繁體中文版本：[上方](#epson-印表機設定指南繁體中文).

## Scope and notes

- Applies to Epson printers with fax, such as the WorkForce and EcoTank L series (for example L5590).
- Menu names vary by model and firmware. If you cannot find the names below, look for **Fax Output**
  or **Save to Network Folder**.
- MyFax does not talk to the printer directly and does not replace the Epson driver. MyFax only
  watches the folders you configure.
- Do not let EPSON FAX Utility and MyFax watch the same folder.
- MyFax watches folders only. It does **not** fetch faxes that are forwarded by e-mail.

## Step 1: Create a shared folder on the PC

1. Create a folder, for example `C:\FaxInbox`.
2. Share it on the network under a share name such as `FaxInbox`.
3. Grant write permission to the Windows account that the printer uses to log in over the network.
   Sharing and permission steps differ between Windows versions and network setups. Grant only the
   permission that is needed.

To check the share, open `\\PC-NAME\FaxInbox` from another PC. You should be able to open the folder
and create a file in it.

## Step 2: Configure the printer

1. Enter the printer's IP address in a browser to open Web Config, or use the printer's control panel.
   Go to the fax **Fax Output** settings.
2. Set **Fax Output** to **Save to Network Folder (SMB)**. Enter a folder shared from this PC, for
   example `\\PC-NAME\FaxInbox`.
3. Choose **PDF** as the file format (recommended). If you choose TIFF, turn on **Also accept TIFF
   faxes** in MyFax.
4. Save the settings.

## Step 3: Add the folder in MyFax

1. Open MyFax and go to **Settings**, then the **Receiving** section.
2. Under **Watch folders**, enter the same folder path that the printer uses, then press **Add**.
3. Adjust **Scan interval (seconds)** if needed. The default is 5 seconds (allowed range 2 to 3600).
4. If the printer saves TIFF files, turn on **Also accept TIFF faxes (.tif / .tiff)**.
5. Press **Scan now** in the status bar to test.

## Step 4: Test

Send a test fax from another fax machine. After the printer writes the file, MyFax scans the folder and
imports it. If nothing appears, see Troubleshooting below.

## How MyFax handles received files

- **Waits for the write to finish**: a file is imported only when its size and modification time are
  unchanged across two scans, it is at least 5 seconds old, and a PDF ends with `%%EOF` (TIFF: valid
  header). A file without a trailer is still imported after 10 unchanged scans.
- **Watch-folder rules**: a watch folder must be an absolute path, must not be inside the archive
  root, and must not equal (or be inside) the extra copy folder.
- **Top level only**: watched folders are read at their top level; subfolders are not entered.
- **Archive location**: the file is copied to
  `<archive root>\YYYY\YYYY-MM\YYYY-MM-DD\YYYYMMDD-HHMMSS_<sender>.pdf`. If there is no sender
  number, `<sender>` is `unknown`.
- **Filename example**: `FAX_20261110_103045_0912345678.pdf` is read as 2026-11-10 10:30:45 from sender
  0912345678. If no valid date is found in the name, the file's modification time is used.
- **Duplicates**: compared by SHA-256. Identical content is archived once; the activity log shows
  "Skipped duplicate fax".
- **Source files**: kept by default. If **Delete the source file after archiving (move instead of copy)**
  is on, the source is removed after a successful import.
- **Extra copy**: if **Also copy new faxes to** is set, each new fax is copied once more to that folder.

## Sending faxes (optional)

Sending uses the **EPSON FAX** driver installed on the PC:

1. Install the Epson FAX driver. Its printer name usually contains "FAX".
2. On MyFax's **Send fax** page, press **Refresh**, then choose the fax printer.
3. Choose the file to send, optionally enter a recipient, then press **Send fax**.
4. MyFax hands the document to the driver, which opens its own window. Enter the fax number in that
   window and send.

MyFax cannot dial for you and cannot confirm delivery. The driver and printer handle the call.

## Troubleshooting

| Symptom | Likely cause | What to do |
|---|---|---|
| "Folder not found: ..." message, or a watcher error in the status bar | Wrong path, or the share is not reachable | Open the path in File Explorer. Fix the path or share permission, then press **Scan now**. |
| The printer reports success, but MyFax shows nothing | Folder not in Watch folders; file is not PDF (or TIFF is off); file still being written or is 0 bytes | Check the watch folder list and the TIFF switch. Wait, then press **Scan now**. |
| The printer cannot write to the shared folder | Insufficient share permission, or the printer's account does not match | Confirm the account the printer uses can write to the folder. |
| A fax appears only once | Duplicate content is skipped | Expected behavior. |
| A received TIFF cannot be previewed | A non-first TIFF page uses non-CCITT compression | Press **Open** to view it in the default app. |
| No printer in the Send fax list | EPSON FAX driver not installed, or the list is stale | Install the driver, then press **Refresh**. |
| Cannot find "Fax Output" on the printer | Menu names differ by model or firmware | Look for **Save to Network Folder** or similar wording. If still not found, see the printer manual. |
