#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc::Auth;
use bitcoin;
use bdk::{Wallet};
use bdk::database::MemoryDatabase;
use bdk::wallet::AddressIndex::New;
use bitcoincore_rpc::Client;
use bdk::blockchain::ConfigurableBlockchain;
use bdk::blockchain::rpc::RpcBlockchain;
use bdk::blockchain::rpc::RpcConfig;
use bdk::blockchain::Blockchain;
use bdk::blockchain::GetHeight;
use std::sync::{Arc, Mutex};
use std::ops::Deref;
use std::process::Command;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;
use home::home_dir;
use secp256k1::{rand, Secp256k1, SecretKey};
use tauri::State;
use std::{thread, time::Duration};



struct TauriState(Mutex<RpcConfig>, Mutex<String>, Mutex<String>, Mutex<String>);

fn write(name: String, value:String) {
	let mut config_file = home_dir().expect("could not get home directory");
    config_file.push("config.txt");
    let mut written = false;
    let mut newfile = String::new();

    let contents = match fs::read_to_string(&config_file) {
        Ok(ct) => ct,
        Err(_) => {
            "".to_string()       
        }
    };

    for line in contents.split("\n") {
        let parts: Vec<&str> = line.split("=").collect();
        if parts.len() == 2 {
           let (n,v) = (parts[0],parts[1]); 
           newfile += n;
           newfile += "=";
           if n == name {
            newfile += &value;
            written = true;
           } else {
            newfile += v;
           }
           newfile += "\n";
        }
    }

    if !written {
        newfile += &name;
        newfile += "=";
        newfile += &value;
    }

    let mut file = File::create(&config_file).expect("Could not Open file");
    file.write_all(newfile.as_bytes()).expect("Could not rewrite file");
}



#[tauri::command]
fn read() -> std::string::String {
    let mut config_file = home_dir().expect("could not get home directory");
    println!("{}", config_file.display());
    config_file.push("config.txt");
    let contents = match fs::read_to_string(&config_file) {
        Ok(ct) => ct,
        Err(_) => {
        	"".to_string()
        }
    };

    for line in contents.split("\n") {
        let parts: Vec<&str> = line.split("=").collect();
        if parts.len() == 2 {
            let (n,v) = (parts[0],parts[1]);
            println!("read line: {}={}", n, v);
        }
    }
    format!("{}", contents)
}

fn generate_private_key() -> Result<bitcoin::PrivateKey, bitcoin::Error> {
	let secp = Secp256k1::new();
	let secret_key = SecretKey::new(&mut rand::thread_rng());
	Ok(bitcoin::PrivateKey::new(secret_key, bitcoin::Network::Bitcoin))
}

fn derive_public_key(private_key: bitcoin::PrivateKey) -> Result<bitcoin::PublicKey, bitcoin::Error>  {
	let secp = Secp256k1::new();
	let secret_key = SecretKey::new(&mut rand::thread_rng());
	Ok(bitcoin::PublicKey::from_private_key(&secp, &private_key))
}

fn store_private_key(private_key: bitcoin::PrivateKey, file_name: String) -> Result<String, String> {
	let mut fileRef = match std::fs::File::create(file_name) {
		Ok(file) => file,
		Err(err) => return Err(err.to_string()),
	};
	fileRef.write_all(&private_key.to_bytes());
	Ok(format!("SUCCESS stored with no problems"))
}

fn store_public_key(public_key: bitcoin::PublicKey, file_name: String) -> Result<String, String> {
	let mut fileRef = match std::fs::File::create(file_name) {
		Ok(file) => file,
		Err(err) => return Err(err.to_string()),
	};
	fileRef.write_all(&public_key.to_bytes());
	Ok(format!("SUCCESS stored with no problems"))
}

#[tauri::command]
async fn generate_store_key_pair(number: String) -> String {
	//number corresponds to currentSD here and is provided by the front end
	let private_key_file = "/mnt/ramdisk/sensitive/private_key".to_string()+&number;
	let public_key_file = "/mnt/ramdisk/sensitive/public_key".to_string()+&number;
	let private_key = match generate_private_key() {
		Ok(private_key) => private_key,
		Err(err) => return "ERROR could not generate private_key: ".to_string()+&err.to_string()
	};
	let public_key = match derive_public_key(private_key) {
		Ok(public_key) => public_key,
		Err(err) => return "ERROR could not dervie public key: ".to_string()+&err.to_string()
	};
	match store_private_key(private_key, private_key_file) {
		Ok(_) => {},
		Err(err) => return "ERROR could not store private key: ".to_string()+&err
	}
	match store_public_key(public_key, public_key_file) {
		Ok(_) => {},
		Err(err) => return "ERROR could not store public key: ".to_string()+&err
	}

	//make the pubkey dir in the setupCD staging area, can fail or succeed
	Command::new("mkdir").args(["--parents", "/mnt/ramdisk/setupCD/pubkeys"]).output().unwrap();

	//copy public key to setupCD dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/sensitive/public_key".to_string()+&number), "/mnt/ramdisk/setupCD/pubkeys"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in generate store key pair with copying pubkey= {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	format!("SUCCESS generated and stored Private and Public Key Pair")
}

fn recover_private_key(file_name: String) -> Result<bitcoin::PrivateKey, String> {
	let private_key_string = match fs::read_to_string(file_name) {
		Ok(data) => data,
		Err(err) => return Err(err.to_string())
	};
	let private_key = match bitcoin::PrivateKey::from_slice(private_key_string.as_bytes(), bitcoin::Network::Bitcoin) {
		Ok(private_key) => private_key,
		Err(err) => return Err(err.to_string())
	};
	Ok(private_key)
}

fn recover_public_key(file_name: String) -> Result<bitcoin::PublicKey, String> {
	let public_key_string = match fs::read_to_string(file_name) {
		Ok(data) => data,
		Err(err) => return Err(err.to_string())
	};
	let public_key = match bitcoin::PublicKey::from_slice(public_key_string.as_bytes()) {
		Ok(public_key) => public_key,
		Err(err) => return Err(err.to_string())
	};
	Ok(public_key)
}

#[tauri::command]
async fn recover_key_pair() -> String {
	let private_key_file = "/mnt/ramdisk/sensitive/private_key.txt".to_string();
	let public_key_file = "/mnt/ramdisk/sensitive/private_key.txt".to_string();
	let private_key = match recover_private_key(private_key_file) {
		Ok(private_key) => private_key,
		Err(err) => return "ERROR could not recover private key: ".to_string()+&err
	};
	let public_key = match recover_public_key(public_key_file) {
		Ok(public_key) => public_key,
		Err(err) => return "ERROR could not recover public key: ".to_string()+&err
	};
	// Use These
	format!("SUCCESS recovered Private/Public Key Pair")
}

// fn build_high_descriptor(blockchain: &RpcBlockchain) -> Result<String, bdk::Error> {
// 	let mut keys = Vec::new();
// 	let ctx = Secp256k1::new();
// 	for i in 0..11 {
// 		keys.push(generate_key().expect("could not get key").public_key(&ctx));
// 		println!("test = {}", generate_key().expect("could not get key").public_key(&ctx));
// 	}
// 	let four_years = blockchain.get_height().unwrap()+210379;
// 	let month = 4382;
// 	let desc = format!("wsh(and_v(v:thresh(5,pk({}),s:pk({}),s:pk({}),s:pk({}),s:pk({}),s:pk({}),s:pk({}),sun:after({}),sun:after({}),sun:after({}),sun:after({})),thresh(2,pk({}),s:pk({}),s:pk({}),s:pk({}),sun:after({}),sun:after({}))))", keys[0], keys[1], keys[2], keys[3], keys[4], keys[5], keys[6], four_years, four_years+(month), four_years+(month*2), four_years+(month*3), keys[7], keys[8], keys[9], keys[10], four_years, four_years);
// 	println!("DESC: {}", desc);
// 	Ok(miniscript::Descriptor::<bitcoin::PublicKey>::from_str(&desc).unwrap().to_string())
// }

// fn build_med_descriptor(blockchain: &RpcBlockchain) -> Result<String, bdk::Error> {
// 	let mut keys = Vec::new();
// 	let ctx = Secp256k1::new();
// 	for i in 0..7 {
// 		keys.push(generate_key().expect("could not get key").public_key(&ctx))
// 	}
// 	let four_years = blockchain.get_height().unwrap()+210379;
// 	let desc = format!("wsh(thresh(2,pk({}),s:pk({}),s:pk({}),s:pk({}),s:pk({}),s:pk({}),s:pk({}),sun:after({})))", keys[0], keys[1], keys[2], keys[3], keys[4], keys[5], keys[6], four_years);
// 	Ok(miniscript::Descriptor::<bitcoin::PublicKey>::from_str(&desc).unwrap().to_string())
// }


// fn build_low_descriptor(blockchain: &RpcBlockchain) -> Result<String, bdk::Error> {
// 	let mut keys = Vec::new();
// 	let ctx = Secp256k1::new();
// 	for i in 0..7 {
// 		keys.push(generate_key().expect("could not get key").public_key(&ctx))
// 	}
// 	let desc = format!("wsh(c:or_i(pk_k({}),or_i(pk_h({}),or_i(pk_h({}),or_i(pk_h({}),or_i(pk_h({}),or_i(pk_h({}),pk_h({}))))))))", keys[0], keys[1], keys[2], keys[3], keys[4], keys[5], keys[6]);
// 	Ok(miniscript::Descriptor::<bitcoin::PublicKey>::from_str(&desc).unwrap().to_string())
// }

// #[tauri::command]
// fn generate_wallet(state: State<TauriState>) -> String {
// 	let blockchain = RpcBlockchain::from_config(&*state.0.lock().unwrap()).expect("failed to connect to bitcoin core(Ensure bitcoin core is running before calling this function)");
// 	*state.1.lock().unwrap() = build_high_descriptor(&blockchain).expect("failed to bulid high lvl descriptor");
// 	*state.2.lock().unwrap() = build_med_descriptor(&blockchain).expect("failed to bulid med lvl descriptor");
// 	*state.3.lock().unwrap() = build_low_descriptor(&blockchain).expect("failed to bulid low lvl descriptor");
// 	return "Completed With No Problems".to_string()
// }

// #[tauri::command]
// fn get_address_high_wallet(state: State<TauriState>) -> String {
// 	println!("test ");
// 	let desc: String = (*state.1.lock().unwrap()).clone();
// 	println!("desc = {}", desc);
// 	let wallet: Wallet<MemoryDatabase> = Wallet::new(&desc, None, bitcoin::Network::Bitcoin, MemoryDatabase::default()).expect("failed to bulid high lvl wallet");
// 	return wallet.get_address(bdk::wallet::AddressIndex::New).expect("could not get address").to_string()
// }


#[tauri::command]
async fn test_function() -> String {
	let file = File::create("/home/".to_string()+&get_user()+"/testfile.txt").unwrap();
	let output = Command::new("echo").args(["file contents go here" ]).stdout(file).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in test function = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in test function")
}


// runs on the boot screen when user clicks install, downloads latest copy of tails
#[tauri::command]
async fn obtain_ubuntu() -> String {
	println!("obtaining & creating modified ubuntu iso");
	let output = Command::new("bash")
           .args(["./scripts/init-iso.sh"])
           .output()
           .expect("failed to execute process");
   for byte in output.stdout {
   	print!("{}", byte as char);
   }
    println!(";");

	format!("completed with no problems")
}

//broken
//initial flash of all 7 SD cards
#[tauri::command]
async fn create_bootable_usb(number:  &str, setup: &str) -> Result<String, String> {
    write("type".to_string(), "sdcard".to_string());
    write("sdNumber".to_string(), number.to_string());
    write("setupStep".to_string(), setup.to_string());
	println!("creating bootable ubuntu device = {} {}", number, setup);
	// sleep for 4 seconds
	Command::new("sleep").args(["4"]).output().unwrap();
	//remove old config from iso
	Command::new("sudo").args(["rm", &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/config.txt")]).output().unwrap();
	//broken?
	//copy new config
	let output = Command::new("sudo").args(["cp", &("/home/".to_string()+&get_user()+"/config.txt"), &("/media".to_string()+&get_user()+"writable/upper/home/ubuntu")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in creating bootable with copying current config = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//open file permissions for config
	let output = Command::new("sudo").args(["chmod", "777", &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/config.txt")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in creating bootable with opening file permissions = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//broken?
	//remove current working config from local
	let output = Command::new("sudo").args(["rm", &("/home/".to_string()+&get_user()+"/config.txt")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in creating bootable with removing current working config = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//this part of the script is broken?
	//burn iso with mkusb
	let output = Command::new("printf").args(["\'%s\n\'", "n", "y", "g", "y", "|", "mkusb", &("/home/".to_string()+&get_user()+"/arctica/persistent-ubuntu.iso")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in creating bootable with mkusb = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	Ok(format!("SUCCESS in creating bootable device"))
}

#[tauri::command]
async fn create_setup_cd() -> String {
	println!("creating setup CD");
	let output = Command::new("bash")
        .args(["/home/".to_string()+&get_user()+"/scripts/create-setup-cd.sh"])
        .output()
        .expect("failed to execute process");
  println!(";");
	format!("{:?}", output)
}

#[tauri::command]
async fn copy_setup_cd() -> String {

	Command::new("mkdir").args(["/mnt/ramdisk/setupCD"]).output().unwrap();

	let output = Command::new("cp").args(["-R", &("/media/".to_string()+&get_user()+"/CDROM"), "/mnt/ramdisk"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in copying setup CD = {}", std::str::from_utf8(&output.stderr).unwrap());
    }
	
	let output = Command::new("mv").args(["/mnt/ramdisk/CDROM", "/mnt/ramdisk/setupCD"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in copying setup CD = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	format!("SUCCESS in coyping setup CD")
}

#[tauri::command]
async fn packup() -> String {
	println!("packing up sensitive info");
	//remove stale encrypted dir
	Command::new("sudo").args(["rm", &("/home/".to_string()+&get_user()+"/encrypted.gpg")]).output().unwrap();

	//remove stale tarball
	Command::new("sudo").args(["rm", "/mnt/ramdisk/unecrypted.tar"]).output().unwrap();

	//pack the sensitive directory into a tarball
	let output = Command::new("tar").args(["cvf", "/mnt/ramdisk/unencrypted.tar", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in packup = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	//encrypt the sensitive directory tarball 
	let output = Command::new("gpg").args(["--batch", "--passphrase-file", "/mnt/ramdisk/masterkey", "--output", &("/home/".to_string()+&get_user()+"/encrypted.gpg"), "--symmetric", "/mnt/ramdisk/unencrypted.tar"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in packup = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	format!("SUCCESS in packup")

}

#[tauri::command]
async fn unpack() -> String {
	println!("unpacking sensitive info");

	//remove stale tarball(We don't care if it fails/succeeds)
	Command::new("sudo").args(["rm", "/mnt/ramdisk/decrypted.out"]).output().unwrap();


	//decrypt sensitive directory
	let output = Command::new("gpg").args(["--batch", "--passphrase-file", "/mnt/ramdisk/masterkey", "--output", "/mnt/ramdisk/decrypted.out", "-d", &("/home/".to_string()+&get_user()+"/encrypted.gpg")]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in unpack = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	// unpack sensitive directory tarball
	let output = Command::new("tar").args(["xvf", "/mnt/ramdisk/decrypted.out", "-C", "/mnt/ramdisk/"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in unpack = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

    // copy sensitive dir to ramdisk
	let output = Command::new("cp").args(["-R", "/mnt/ramdisk/mnt/ramdisk/sensitive", "/mnt/ramdisk"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in unpack = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	// remove nested sensitive
	Command::new("sudo").args(["rm", "-r", "/mnt/ramdisk/mnt"]).output().unwrap();

	// #NOTES:
	// #use this to append files to a decrypted tarball without having to create an entire new one
	// #tar rvf output_tarball ~/filestobeappended
	format!("SUCCESS in unpack")
}

#[tauri::command]
async fn create_ramdisk() -> String {
	println!("creating ramdisk");

	Command::new("sudo").args(["mkdir", "/mnt/ramdisk"]).output().unwrap();

	let output = Command::new("sudo").args(["mount", "-t", "ramfs", "-o", "size=250M", "ramfs", "/mnt/ramdisk"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in Creating Ramdisk = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	let output = Command::new("sudo").args(["chmod", "777", "/mnt/ramdisk"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in Creating Ramdisk = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	//make the target dir for encrypted payload to or from SD cards
	Command::new("mkdir").args(["/mnt/ramdisk/sensitive"]).output().unwrap();

	format!("SUCCESS in Creating Ramdisk")
}

#[tauri::command]
fn read_cd() -> std::string::String {
    // sleep for 4 seconds
	Command::new("sleep").args(["4"]).output().unwrap();
    let config_file = "/media/".to_string()+&get_user()+"/CDROM/config.txt";
    let contents = match fs::read_to_string(&config_file) {
        Ok(ct) => ct,
        Err(_) => {
        	"".to_string()
        }
    };

    for line in contents.split("\n") {
        let parts: Vec<&str> = line.split("=").collect();
        if parts.len() == 2 {
            let (n,v) = (parts[0],parts[1]);
            println!("read line: {}={}", n, v);
        }
    }
    format!("{}", contents)
}

//helper function
fn print_rust(data: &str) -> String {
	println!("input = {}", data);
	format!("completed with no problems")
}

//helper function
fn get_user() -> String {
	home_dir().unwrap().to_str().unwrap().to_string().split("/").collect::<Vec<&str>>()[2].to_string()
}

#[tauri::command]
async fn combine_shards() -> String {
	println!("combining shards in /mnt/ramdisk/shards");
	let output = Command::new("bash")
		.args(["/home/".to_string()+&get_user()+"/scripts/combine-shards.sh"])
		.output()
		.expect("failed to execute process");
	format!("{:?}", output)
}

#[tauri::command]
async fn async_write(name: &str, value: &str) -> Result<String, String> {
    write(name.to_string(), value.to_string());
    println!("{}", name);
    Ok(format!("completed with no problems"))
}

#[tauri::command]
async fn mount_internal() -> String {
	println!("mounting internal storage and symlinking .bitcoin dirs");
	let output = Command::new("bash")
		.args(["/home/".to_string()+&get_user()+"/scripts/mount-internal.sh"])
		.output()
		.expect("failed to execute process");
	format!("{:?}", output)
}

#[tauri::command]
async fn install_sd_deps() -> String {
	println!("installing deps required by SD card");
	//these are required on all 7 SD cards and will eventually be installed prior to first boot
	let output = Command::new("sudo").args(["add-apt-repository", "-y", "universe"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in installing SD dependencies = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	let output = Command::new("sudo").args(["apt", "update"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in installing SD dependencies = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	//download wodim
	let output = Command::new("sudo").args(["apt", "install", "-y", "wodim"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in installing SD dependencies = {}", std::str::from_utf8(&output.stderr).unwrap());
    }
	//download shamir secret sharing library
	let output = Command::new("sudo").args(["apt", "install", "ssss"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in installing SD dependencies = {}", std::str::from_utf8(&output.stderr).unwrap());
    }

	format!("SUCCESS in installing SD dependencies")
}

#[tauri::command]
async fn refresh_setup_cd() -> String {
	//create iso from setupCD dir
	let output = Command::new("genisoimage").args(["-r", "-J", "-o", "/mnt/ramdisk/setupCD.iso", "/mnt/ramdisk/setupCD"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR refreshing setupCD with genisoimage = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	//wipe the CD
	Command::new("sudo").args(["umount", "/dev/sr0"]).output().unwrap();
	let output = Command::new("wodim").args(["-v", "dev=/dev/sr0", "blank=fast"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR refreshing setupCD with wiping CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	//burn setupCD iso to the setupCD
	let output = Command::new("wodim").args(["dev=/dev/sr0", "-v", "-data", "/mnt/ramdisk/setupCD.iso"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in refreshing setupCD with burning iso = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	//eject the disc
	let output = Command::new("eject").args(["/dev/sr0"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in refreshing setupCD with ejecting CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in refreshing setupCD")
}

#[tauri::command]
async fn distribute_shards_sd2() -> String {
	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard2.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd2 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard10.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd2 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in distributing shards to SD 2")
}

#[tauri::command]
async fn distribute_shards_sd3() -> String {
	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard3.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd3 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard9.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd3 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in distributing shards to SD 3")
}

#[tauri::command]
async fn distribute_shards_sd4() -> String {
	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard4.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd4 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard8.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd4 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in distributing shards to SD 4")
}

#[tauri::command]
async fn distribute_shards_sd5() -> String {
	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard5.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd5 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in distributing shards to SD 5")
}

#[tauri::command]
async fn distribute_shards_sd6() -> String {
	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard6.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd6 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in distributing shards to SD 6")
}

#[tauri::command]
async fn distribute_shards_sd7() -> String {
	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/shards/shard7.txt", &("/home".to_string()+&get_user()+"/shards")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in distributing shards to sd7 = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in distributing shards to SD 7")
}

//deprecated
#[tauri::command]
async fn create_descriptor() -> String {
	println!("creating descriptor from 7 xpubs");
	let output = Command::new("bash")
		.args(["/home/".to_string()+&get_user()+"/scripts/create-descriptor.sh"])
		.output()
		.expect("failed to execute process");
	format!("{:?}", output)
}

//deprecated
#[tauri::command]
async fn copy_descriptor() -> String {
	fs::copy("/mnt/ramdisk/setupCD/descriptor.txt", "/mnt/ramdisk/sensitive/descriptor.txt");
	format!("completed with no problems")
	
}

#[tauri::command]
async fn extract_masterkey() -> String {
	let output = Command::new("cp").args(["/mnt/ramdisk/setupCD/masterkey", "/mnt/ramdisk"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in extracting masterkey = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in extracting masterkey")
}

#[tauri::command]
async fn create_backup(number: String) -> String {
	println!("creating backup directory of the current SD");
		//make backup dir for iso
		Command::new("mkdir").args(["/mnt/ramdisk/backup"]).output().unwrap();
		//Copy shards to backup
		let output = Command::new("cp").args(["-r", &("/home/".to_string()+&get_user()+"/shards"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			// Function Fails
			return format!("ERROR in creating backup with copying shards = {}", std::str::from_utf8(&output.stderr).unwrap());
		}
		//Copy sensitive dir
		let output = Command::new("cp").args([&("/home/".to_string()+&get_user()+"/encrypted.gpg"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			// Function Fails
			return format!("ERROR in creating backup with copying sensitive dir= {}", std::str::from_utf8(&output.stderr).unwrap());
		}
		//copy config
		let output = Command::new("cp").args([&("/home/".to_string()+&get_user()+"/config.txt"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			// Function Fails
			return format!("ERROR in creating backup with copying config.txt= {}", std::str::from_utf8(&output.stderr).unwrap());
		}
		//create .iso from backup dir
		let output = Command::new("genisoimage").args(["-r", "-J", "-o", &("/mnt/ramdisk/backup".to_string()+&number+".iso"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			// Function Fails
			return format!("ERROR in creating backup with creating iso= {}", std::str::from_utf8(&output.stderr).unwrap());
		}
	
		format!("SUCCESS in creating backup of current SD")
}

#[tauri::command]
async fn make_backup(number: String) -> String {
	println!("making backup iso of the current SD and burning to CD");
	// sleep for 4 seconds
	Command::new("sleep").args(["4"]).output().unwrap();
	//wipe the CD
	Command::new("sudo").args(["umount", "/dev/sr0"]).output().unwrap();
	let output = Command::new("wodim").args(["-v", "dev=/dev/sr0", "blank=fast"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in making backup with wiping CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//burn setupCD iso to the backup CD
	let output = Command::new("wodim").args(["dev=/dev/sr0", "-v", "-data", &("/mnt/ramdisk/backup".to_string()+&number+".iso")]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in making backup with burning iso to CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//eject the disc
	let output = Command::new("eject").args(["/dev/sr0"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in refreshing setupCD with ejecting CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in making backup of current SD")
}

#[tauri::command]
async fn start_bitcoind() -> String {
	println!("starting the bitcoin daemon");
	let output = Command::new("bash").args(["/home/".to_string()+&get_user()+"/bitcoin-23.0/bin/bitcoind"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in starting bitcoin daemon = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in starting bitcoin daemon")
}

#[tauri::command]
async fn start_bitcoind_network_off() -> String {
	println!("starting the bitcoin daemon with networking disabled");
	let output = Command::new("bash").args([&("/home/".to_string()+&get_user()+"/bitcoin-23.0/bin/bitcoind"), "-networkactive=0"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in starting bitcoin daemon with networking disabled = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in starting bitcoin daemon with networking disabled")
}

#[tauri::command]
async fn check_for_masterkey() -> String {
	println!("checking ramdisk for masterkey");
    let b = std::path::Path::new("/mnt/ramdisk/masterkey").exists();
    if b == true{
        format!("masterkey found")
    }
	else{
        format!("key not found")
    }
}

#[tauri::command]
async fn retrieve_masterkey() -> String {
	println!("checking transferCD for masterkey");
    let b = std::path::Path::new(&("/media/".to_string()+&get_user()+"/CDROM/masterkey")).exists();
    if b == true{
		let output = Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/masterkey"), "/mnt/ramdisk"]).output().unwrap();
		if !output.status.success() {
			// Function Fails
			return format!("ERROR in retrieving masterkey = {}", std::str::from_utf8(&output.stderr).unwrap());
		}
        format!("masterkey found")
    }
	else{
        format!("key not found")
    }
}

#[tauri::command]
async fn create_recovery_cd() -> String {
	println!("creating recovery CD for manual decrypting");
	let output = Command::new("bash")
		.args(["/home/".to_string()+&get_user()+"/scripts/create-recovery-cd.sh"])
		.output()
		.expect("failed to execute process");
	format!("{:?}", output)
}

#[tauri::command]
async fn copy_recovery_cd() -> String {
	Command::new("mkdir").args(["/mnt/ramdisk/shards"]).output().unwrap();
	let output = Command::new("bash")
        .args(["/home/".to_string()+&get_user()+"/scripts/copy-recovery-cd.sh"])
        .output()
        .expect("failed to execute process");
  println!(";");
	format!("{:?}", output)
}

#[tauri::command]
async fn calculate_number_of_shards_cd() -> u32 {
	let mut x = 0;
    for file in fs::read_dir("/media/".to_string()+&get_user()+"/CDROM/shards").unwrap() {
		x = x + 1;
	}
	return x;
}

#[tauri::command]
async fn calculate_number_of_shards_ramdisk() -> u32 {
	let mut x = 0;
    for file in fs::read_dir("/mnt/ramdisk/transferCD/shards").unwrap() {
		x = x + 1;
	}
	return x;
}


//broken
#[tauri::command]
async fn collect_shards() -> String {
	println!("collecting shards");
	//create transferCD target dir
	Command::new("mkdir").args(["--parents", "/mnt/ramdisk/transferCD/shards"]).output().unwrap();
	//stage transferCD target dir with current CD content
	let output = Command::new("cp").args(["-R", &("/media/".to_string()+&get_user()+"/CDROM"), "/mnt/ramdisk"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR in collecting shards with copying CDROM contents = {}", std::str::from_utf8(&output.stderr).unwrap());
    }
	let output = Command::new("mv").args(["/mnt/ramdisk/CDROM", "/mnt/ramdisk/transferCD"]).output().unwrap();
	if !output.status.success() {
    	// Function Fails
    	return format!("ERROR collecting shards with moving CDROM contents = {}", std::str::from_utf8(&output.stderr).unwrap());
    }
	//create transferCD config
	let file = File::create("/mnt/ramdisk/transferCD/config.txt").unwrap();
	let output = Command::new("echo").args(["type=transfercd" ]).stdout(file).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in converting to transfer CD, with creating config = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	//this entire function is currently broken until a solution for the below recursive copy is discovered
	//collect shards from sd card for export to transfer CD
	//cp -r /home/$USER/shards/asterisk /mnt/ramdisk/transferCD/shards

	//create iso from transferCD dir
	let output = Command::new("genisoimage").args(["-r", "-J", "-o", "/mnt/ramdisk/transferCD.iso", "/mnt/ramdisk/transferCD"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR converting to transfer CD with wiping CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//wipe the CD
	Command::new("sudo").args(["umount", "/dev/sr0"]).output().unwrap();
	let output = Command::new("wodim").args(["-v", "dev=/dev/sr0", "blank=fast"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR converting to transfer CD with wiping CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//burn transferCD iso to the transfer CD
	Command::new("wodim").args(["dev=/dev/sr0", "-v", "-data", "/mnt/ramdisk/transferCD.iso"]).output().unwrap();
	let output = Command::new("wodim").args(["-v", "dev=/dev/sr0", "blank=fast"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR converting to transfer CD with wiping CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//eject the disc
	let output = Command::new("eject").args(["/dev/sr0"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in refreshing setupCD with ejecting CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in collecting shards")
	
}

#[tauri::command]
async fn convert_to_transfer_cd() -> String {
	println!("converting completed recovery cd to transfer cd with masterkey");
	//create transferCD target dir
	Command::new("mkdir").args(["/mnt/ramdisk/transferCD"]).output().unwrap();
	//create transferCD config
	let file = File::create("/mnt/ramdisk/transferCD/config.txt").unwrap();
	let output = Command::new("echo").args(["type=transfercd" ]).stdout(file).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in converting to transfer CD, with creating config = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//collect masterkey from cd dump and prepare for transfer to transfercd
	let output = Command::new("cp").args(["/mnt/ramdisk/masterkey", "/mnt/ramdisk/transferCD"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in coverting to transfer CD, with copying masterkey = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//create iso from transferCD dir
	let output = Command::new("genisoimage").args(["-r", "-J", "-o", "/mnt/ramdisk/transferCD.iso", "/mnt/ramdisk/transferCD"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in converting to transfer CD, with copying masterkey = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//wipe the CD
	Command::new("sudo").args(["umount", "/dev/sr0"]).output().unwrap();
	let output = Command::new("wodim").args(["-v", "dev=/dev/sr0", "blank=fast"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR converting to transfer CD with wiping CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//burn transferCD iso to the transfer CD
	let output = Command::new("wodim").args(["dev=/dev/sr0", "-v", "-data", "/mnt/ramdisk/transferCD/iso"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR refreshing setupCD with wiping CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//eject the disc
	let output = Command::new("eject").args(["/dev/sr0"]).output().unwrap();
	if !output.status.success() {
		// Function Fails
		return format!("ERROR in refreshing setupCD with ejecting CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in converting to transfer CD")
}

fn main() {
	//TODO: confirm all these strings are correct per user(parse the bitcoin.conf)
	let user_pass: bdk::blockchain::rpc::Auth = bdk::blockchain::rpc::Auth::UserPass{username: "rpcuser".to_string(), password: "477028".to_string()};
    let config: RpcConfig = RpcConfig {
	    url: "127.0.0.1:8332".to_string(),
	    auth: user_pass,
	    network: bdk::bitcoin::Network::Bitcoin,
	    wallet_name: "wallet_name".to_string(),
	    sync_params: None,
	};
  	tauri::Builder::default()
  	.manage(TauriState(Mutex::new(config), Mutex::new("".to_string()), Mutex::new("".to_string()), Mutex::new("".to_string())))
  	.invoke_handler(tauri::generate_handler![
        test_function,
        create_bootable_usb,
        create_setup_cd,
        read_cd,
        copy_setup_cd,
        obtain_ubuntu,
        async_write,
        read,
        combine_shards,
        mount_internal,
        create_ramdisk,
        packup,
        unpack,
        install_sd_deps,
        refresh_setup_cd,
        distribute_shards_sd2,
        distribute_shards_sd3,
        distribute_shards_sd4,
        distribute_shards_sd5,
        distribute_shards_sd6,
        distribute_shards_sd7,
        create_descriptor,
        copy_descriptor,
        extract_masterkey,
        create_backup,
        make_backup,
        start_bitcoind,
        start_bitcoind_network_off,
        check_for_masterkey,
		retrieve_masterkey,
        create_recovery_cd,
        copy_recovery_cd,
        calculate_number_of_shards_cd,
		calculate_number_of_shards_ramdisk,
        collect_shards,
        convert_to_transfer_cd,
		generate_store_key_pair,
		recover_key_pair,
        ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}