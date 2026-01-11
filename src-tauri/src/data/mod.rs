use crate::util::{current_unix_timestamp, generate_random_string, get_base_directory};
use crate::demucs::{detect_bpm, detect_key};
use polodb_core::{bson::doc, Collection, Database};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use self::fsio::{copy_song_to_project, delete_project_data};

mod fsio;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AppMetadata {
    #[serde(alias = "activation")]
    Activation { key: String },

    #[serde(alias = "num_songs_processed")]
    Error { value: u32 },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub _id: String,
    pub name: String,
    pub created_at: i64,
    pub base_dir: PathBuf,
    pub stem_paths: Vec<String>,
    #[serde(default)]
    pub bpm: Option<f64>,
    #[serde(default)]
    pub key: Option<String>,
}

pub struct AppDb {
    pub path: PathBuf,
    polo_instance: Database,
}

// TODO: Implement non-monkey error handling
impl AppDb {
    pub fn new(path: PathBuf) -> Self {
        let db = Database::open_file(path.clone()).unwrap();
        Self {
            path: path.clone(),
            polo_instance: db,
        }
    }

    pub fn create_project(&self, audio_filepath: PathBuf) -> Result<Project, String> {
        let name = audio_filepath
            .file_name()
            .ok_or_else(String::new)?
            .to_string_lossy()
            .to_string();

        let created_at = current_unix_timestamp();
        let projects = self.polo_instance.collection("projects");
        let base_dir = get_base_directory();
        let id = generate_random_string();
        let stem_paths: Vec<String> = vec![];
        let base_dir_clone = base_dir.clone();

        let proj = Project {
            _id: id.clone(), // Not sure if polo_db will work if this is an Option<T>
            name,
            created_at,
            base_dir: base_dir_clone.clone(),
            stem_paths,
            bpm: None,
            key: None,
        };

        projects
            .insert_one(proj.clone())
            .map_err(|_| String::new())?;
        copy_song_to_project(audio_filepath.clone(), id.clone()).expect("Failed to copy song");

        // BPMとKeyを計算してProjectを更新
        let project_dir = base_dir_clone.join("project_data").join(id.clone());
        let audio_path = project_dir.join(
            audio_filepath
                .extension()
                .map(|ext| format!("main.{}", ext.to_string_lossy()))
                .unwrap_or_else(|| "main.mp3".to_string())
        );

        // BPMとKeyを計算してProjectを更新（エラーログを追加）
        eprintln!("[create_project] Detecting BPM and Key for: {:?}", audio_path);
        let bpm_result = detect_bpm(&audio_path);
        let key_result = detect_key(&audio_path);
        
        match &bpm_result {
            Ok(Some(bpm)) => eprintln!("[create_project] BPM detected: {}", bpm),
            Ok(None) => eprintln!("[create_project] BPM detection returned None"),
            Err(e) => eprintln!("[create_project] BPM detection error: {:?}", e),
        }
        
        match &key_result {
            Ok(Some(key)) => eprintln!("[create_project] Key detected: {}", key),
            Ok(None) => eprintln!("[create_project] Key detection returned None"),
            Err(e) => eprintln!("[create_project] Key detection error: {:?}", e),
        }
        
        let bpm = bpm_result.ok().flatten();
        let key = key_result.ok().flatten();

        // BPMとKeyを更新（Noneでも更新を試みる）
        let projects_collection: Collection<Project> = self.polo_instance.collection("projects");
        let mut update_doc = doc! {};
        
        // BPMが検出された場合、更新ドキュメントに追加
        if let Some(bpm_val) = bpm {
            update_doc.insert("bpm", bpm_val);
            eprintln!("[create_project] Adding BPM to update: {}", bpm_val);
        } else {
            eprintln!("[create_project] BPM is None, skipping BPM update");
        }
        
        // Keyが検出された場合、更新ドキュメントに追加
        if let Some(key_val) = &key {
            update_doc.insert("key", key_val);
            eprintln!("[create_project] Adding Key to update: {}", key_val);
        } else {
            eprintln!("[create_project] Key is None, skipping Key update");
        }
        
        // 更新ドキュメントが空でない場合のみ、データベースを更新
        if !update_doc.is_empty() {
            eprintln!("[create_project] Updating database with: {:?}", update_doc);
            match projects_collection.update_one(
                doc! { "_id": id.clone() },
                doc! { "$set": update_doc.clone() },
            ) {
                Ok(_) => {
                    eprintln!("[create_project] Database update successful for project ID: {}", id);
                }
                Err(e) => {
                    eprintln!("[create_project] ERROR: Failed to update BPM and Key in database: {:?}", e);
                    eprintln!("[create_project] Update document was: {:?}", update_doc);
                    // エラーを返すのではなく、警告だけを出す（プロジェクト作成は続行）
                    // BPM/Keyの更新失敗は致命的ではないため、プロジェクト作成は成功として扱う
                }
            }
        } else {
            eprintln!("[create_project] WARNING: No BPM or Key to update (both are None or empty)");
        }

        // 更新されたProjectを取得
        let updated_proj = projects_collection
            .find_one(doc! { "_id": id.clone() })
            .map_err(|_| String::from("Failed to find updated project"))?
            .ok_or_else(|| String::from("Project not found after update"))?;

        eprintln!("[create_project] Project created with BPM: {:?}, Key: {:?}", updated_proj.bpm, updated_proj.key);

        Ok(updated_proj)
    }

    pub fn add_stems_to_project(
        &self,
        project_id: String,
        stem_paths: Vec<PathBuf>,
    ) -> Result<(), String> {
        let paths: Vec<String> = stem_paths
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let projects: Collection<Project> = self.polo_instance.collection("projects");
        let result = projects.update_one(
            doc! { "_id": project_id.clone() },
            doc! {
                "$set": doc! {
                    // "stem_paths": paths.into_iter().map(Bson::String).collect(),
                    "stem_paths": paths.clone(),
                }
            },
        );

        result.map_err(|_| String::new())?;

        Ok(())
    }

    pub fn get_projects(&self) -> Result<Vec<Project>, String> {
        let projects_collection: Collection<Project> = self.polo_instance.collection("projects");
        let result = projects_collection.find(None);
        match result {
            Ok(res) => {
                let mut all_projects: Vec<Project> = vec![];
                for proj_res in res {
                    let project = proj_res.expect("Couldn't read the project.");
                    all_projects.push(project);
                }
                Ok(dbg!(all_projects))
            }
            Err(_) => Err(String::from("bruh")),
        }
    }

    pub fn get_project_by_id(&self, id: String) -> Result<Option<Project>, String> {
        let projects_collection: Collection<Project> = self.polo_instance.collection("projects");
        let find_result = projects_collection.find_one(doc! {
            "_id": id
        });

        match find_result {
            Ok(result) => Ok(result),
            Err(_e) => Err(String::from("Error finding project by ID")),
        }
    }

    pub fn delete_project_by_id(&self, project_id: String) -> Result<(), String> {
        let projects_collection: Collection<Project> = self.polo_instance.collection("projects");
        let deleted_result = projects_collection.delete_many(doc! {
            "_id": project_id.clone(),
        });

        match deleted_result {
            Ok(_) => {
                delete_project_data(project_id.clone()).expect("Failed to delete project data.");
                Ok(())
            }
            Err(_) => Err(String::from("Error deleting, whoops.")),
        }
    }
}
