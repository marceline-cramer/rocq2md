(* # Introduction

This module defines a trivial lemma. --#-- (internal note, remove) --#-- Enjoy! *)

Definition foo := 1. (* not converted: inline trailing comment *)

 (* not converted: does not start at columnn 0 *)
 
Lemma foo_eq : foo = 1.
Proof.
  (* This comment is inside a proof and stays as code, no warning. *)
  reflexivity.
Qed.
