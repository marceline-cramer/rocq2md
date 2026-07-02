(* # Factorial

Let us introduce the factorial function. --#-- side comment, removed from comversion ---#-- It is defined recursively. *)

Fixpoint fact (n : nat) := (* not converted: inline trailing comment *)
  match n with
  | 0   => 1
  | S n' => n * fact n'  (* note the role of `n'` here --#-- extra note: this comment should be removed --#-- *)
  end.

  (* not converted: does not start at columnn 0 *)

Compute fact 5.

Lemma fact_succ (n' : nat) : fact (S n') = S n' * fact n'.
Proof.
  reflexivity.
  (* the proof holds by reflexivity --#-- this comment is inside a proof and is not converted to text --#-- *)
Qed.
